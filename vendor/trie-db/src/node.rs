// Copyright 2017, 2021 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
	nibble::{self, nibble_ops, NibbleSlice, NibbleVec},
	node_codec::NodeCodec,
	Bytes, CError, ChildReference, Result, TrieError, TrieHash, TrieLayout,
};
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec::Vec};
use hash_db::Hasher;

use crate::rstd::{borrow::Borrow, mem, ops::Range};

/// Partial node key type: offset and owned value of a nibbleslice.
/// Offset is applied on first byte of array (bytes are right aligned).
pub type NodeKey = (usize, nibble::BackingByteVec);

/// A reference to a trie node which may be stored within another trie node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeHandle<'a> {
	Hash(&'a [u8]),
	Inline(&'a [u8]),
}

impl NodeHandle<'_> {
	/// Converts this node handle into a [`NodeHandleOwned`].
	pub fn to_owned_handle<L: TrieLayout>(
		&self,
	) -> Result<NodeHandleOwned<TrieHash<L>>, TrieHash<L>, CError<L>> {
		match self {
			Self::Hash(h) => decode_hash::<L::Hash>(h)
				.ok_or_else(|| Box::new(TrieError::InvalidHash(Default::default(), h.to_vec())))
				.map(NodeHandleOwned::Hash),
			Self::Inline(i) => match L::Codec::decode(i) {
				Ok(node) => Ok(NodeHandleOwned::Inline(Box::new(node.to_owned_node::<L>()?))),
				Err(e) => Err(Box::new(TrieError::DecoderError(Default::default(), e))),
			},
		}
	}
}

/// Owned version of [`NodeHandleOwned`].
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum NodeHandleOwned<H> {
	Hash(H),
	Inline(Box<NodeOwned<H>>),
}

impl<H> NodeHandleOwned<H>
where
	H: Default + AsRef<[u8]> + AsMut<[u8]> + Copy,
{
	/// Returns `self` as a [`ChildReference`].
	///
	/// # Panic
	///
	/// This function panics if `self == Self::Inline(_)` and the inline node encoded length is
	/// greater then the length of the hash.
	fn as_child_reference<C: NodeCodec<HashOut = H>>(&self) -> ChildReference<H> {
		match self {
			NodeHandleOwned::Hash(h) => ChildReference::Hash(*h),
			NodeHandleOwned::Inline(n) => {
				let encoded = n.to_encoded::<C>();
				let mut store = H::default();
				assert!(store.as_ref().len() > encoded.len(), "Invalid inline node handle");

				store.as_mut()[..encoded.len()].copy_from_slice(&encoded);
				ChildReference::Inline(store, encoded.len())
			},
		}
	}
}

impl<H> NodeHandleOwned<H> {
	/// Returns `self` as inline node.
	pub fn as_inline(&self) -> Option<&NodeOwned<H>> {
		match self {
			Self::Hash(_) => None,
			Self::Inline(node) => Some(&*node),
		}
	}
}

/// Read a hash from a slice into a Hasher output. Returns None if the slice is the wrong length.
pub fn decode_hash<H: Hasher>(data: &[u8]) -> Option<H::Out> {
	if data.len() != H::LENGTH {
		return None
	}
	let mut hash = H::Out::default();
	hash.as_mut().copy_from_slice(data);
	Some(hash)
}

/// Value representation in `Node`.
#[derive(Eq, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Value<'a> {
	/// Value byte slice as stored in a trie node.
	Inline(&'a [u8]),
	/// Hash byte slice as stored in a trie node.
	Node(&'a [u8]),
}

impl<'a> Value<'a> {
	pub(crate) fn new_inline(value: &'a [u8], threshold: Option<u32>) -> Option<Self> {
		if let Some(threshold) = threshold {
			if value.len() >= threshold as usize {
				return None
			} else {
				Some(Value::Inline(value))
			}
		} else {
			Some(Value::Inline(value))
		}
	}

	pub fn to_owned_value<L: TrieLayout>(&self) -> ValueOwned<TrieHash<L>> {
		match self {
			Self::Inline(data) => ValueOwned::Inline(Bytes::from(*data), L::Hash::hash(data)),
			Self::Node(hash) => {
				let mut res = TrieHash::<L>::default();
				res.as_mut().copy_from_slice(hash);

				ValueOwned::Node(res)
			},
		}
	}
}

/// Owned value representation in `Node`.
#[derive(Eq, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum ValueOwned<H> {
	/// Value bytes as stored in a trie node and its hash.
	Inline(Bytes, H),
	/// Hash byte slice as stored in a trie node.
	Node(H),
}

impl<H: AsRef<[u8]> + Copy> ValueOwned<H> {
	/// Returns self as [`Value`].
	pub fn as_value(&self) -> Value {
		match self {
			Self::Inline(data, _) => Value::Inline(&data),
			Self::Node(hash) => Value::Node(hash.as_ref()),
		}
	}

	/// Returns the hash of the data stored in self.
	pub fn data_hash(&self) -> Option<H> {
		match self {
			Self::Inline(_, hash) => Some(*hash),
			Self::Node(hash) => Some(*hash),
		}
	}
}

impl<H> ValueOwned<H> {
	/// Returns the data stored in self.
	pub fn data(&self) -> Option<&Bytes> {
		match self {
			Self::Inline(data, _) => Some(data),
			Self::Node(_) => None,
		}
	}
}

/// Type of node in the trie and essential information thereof.
#[derive(Eq, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Node<'a> {
	/// Null trie node; could be an empty root or an empty branch entry.
	Empty,
	/// Leaf node; has key slice and value. Value may not be empty.
	Leaf(NibbleSlice<'a>, Value<'a>),
	/// Extension node; has key slice and node data. Data may not be null.
	Extension(NibbleSlice<'a>, NodeHandle<'a>),
	/// Branch node; has slice of child nodes (each possibly null)
	/// and an optional immediate node data.
	Branch([Option<NodeHandle<'a>>; nibble_ops::NIBBLE_LENGTH], Option<Value<'a>>),
	/// Branch node with support for a nibble (when extension nodes are not used).
	NibbledBranch(
		NibbleSlice<'a>,
		[Option<NodeHandle<'a>>; nibble_ops::NIBBLE_LENGTH],
		Option<Value<'a>>,
	),
}

impl Node<'_> {
	/// Converts this node into a [`NodeOwned`].
	pub fn to_owned_node<L: TrieLayout>(
		&self,
	) -> Result<NodeOwned<TrieHash<L>>, TrieHash<L>, CError<L>> {
		match self {
			Self::Empty => Ok(NodeOwned::Empty),
			Self::Leaf(n, d) => Ok(NodeOwned::Leaf((*n).into(), d.to_owned_value::<L>())),
			Self::Extension(n, h) =>
				Ok(NodeOwned::Extension((*n).into(), h.to_owned_handle::<L>()?)),
			Self::Branch(childs, data) => {
				let mut childs_owned = [(); nibble_ops::NIBBLE_LENGTH].map(|_| None);
				childs
					.iter()
					.enumerate()
					.map(|(i, c)| {
						childs_owned[i] =
							c.as_ref().map(|c| c.to_owned_handle::<L>()).transpose()?;
						Ok(())
					})
					.collect::<Result<(), _, _>>()?;

				Ok(NodeOwned::Branch(childs_owned, data.as_ref().map(|d| d.to_owned_value::<L>())))
			},
			Self::NibbledBranch(n, childs, data) => {
				let mut childs_owned = [(); nibble_ops::NIBBLE_LENGTH].map(|_| None);
				childs
					.iter()
					.enumerate()
					.map(|(i, c)| {
						childs_owned[i] =
							c.as_ref().map(|c| c.to_owned_handle::<L>()).transpose()?;
						Ok(())
					})
					.collect::<Result<(), _, _>>()?;

				Ok(NodeOwned::NibbledBranch(
					(*n).into(),
					childs_owned,
					data.as_ref().map(|d| d.to_owned_value::<L>()),
				))
			},
		}
	}
}

/// Owned version of [`Node`].
#[derive(Eq, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum NodeOwned<H> {
	/// Null trie node; could be an empty root or an empty branch entry.
	Empty,
	/// Leaf node; has key slice and value. Value may not be empty.
	Leaf(NibbleVec, ValueOwned<H>),
	/// Extension node; has key slice and node data. Data may not be null.
	Extension(NibbleVec, NodeHandleOwned<H>),
	/// Branch node; has slice of child nodes (each possibly null)
	/// and an optional immediate node data.
	Branch([Option<NodeHandleOwned<H>>; nibble_ops::NIBBLE_LENGTH], Option<ValueOwned<H>>),
	/// Branch node with support for a nibble (when extension nodes are not used).
	NibbledBranch(
		NibbleVec,
		[Option<NodeHandleOwned<H>>; nibble_ops::NIBBLE_LENGTH],
		Option<ValueOwned<H>>,
	),
	/// Node that represents a value.
	///
	/// This variant is only constructed when working with a [`crate::TrieCache`]. It is only
	/// used to cache a raw value.
	Value(Bytes, H),
}

impl<H> NodeOwned<H>
where
	H: Default + AsRef<[u8]> + AsMut<[u8]> + Copy,
{
	/// Convert to its encoded format.
	pub fn to_encoded<C>(&self) -> Vec<u8>
	where
		C: NodeCodec<HashOut = H>,
	{
		match self {
			Self::Empty => C::empty_node().to_vec(),
			Self::Leaf(partial, value) =>
				C::leaf_node(partial.right_iter(), partial.len(), value.as_value()),
			Self::Extension(partial, child) => C::extension_node(
				partial.right_iter(),
				partial.len(),
				child.as_child_reference::<C>(),
			),
			Self::Branch(children, value) => C::branch_node(
				children.iter().map(|child| child.as_ref().map(|c| c.as_child_reference::<C>())),
				value.as_ref().map(|v| v.as_value()),
			),
			Self::NibbledBranch(partial, children, value) => C::branch_node_nibbled(
				partial.right_iter(),
				partial.len(),
				children.iter().map(|child| child.as_ref().map(|c| c.as_child_reference::<C>())),
				value.as_ref().map(|v| v.as_value()),
			),
			Self::Value(data, _) => data.to_vec(),
		}
	}

	/// Returns an iterator over all existing children with their optional nibble.
	pub fn child_iter(&self) -> impl Iterator<Item = (Option<u8>, &NodeHandleOwned<H>)> {
		enum ChildIter<'a, H> {
			Empty,
			Single(&'a NodeHandleOwned<H>, bool),
			Array(&'a [Option<NodeHandleOwned<H>>; nibble_ops::NIBBLE_LENGTH], usize),
		}

		impl<'a, H> Iterator for ChildIter<'a, H> {
			type Item = (Option<u8>, &'a NodeHandleOwned<H>);

			fn next(&mut self) -> Option<Self::Item> {
				loop {
					match self {
						Self::Empty => break None,
						Self::Single(child, returned) =>
							break if *returned {
								None
							} else {
								*returned = true;
								Some((None, child))
							},
						Self::Array(childs, index) =>
							if *index >= childs.len() {
								break None
							} else {
								*index += 1;

								// Ignore non-existing childs.
								if let Some(ref child) = childs[*index - 1] {
									break Some((Some(*index as u8 - 1), child))
								}
							},
					}
				}
			}
		}

		match self {
			Self::Leaf(_, _) | Self::Empty | Self::Value(_, _) => ChildIter::Empty,
			Self::Extension(_, child) => ChildIter::Single(child, false),
			Self::Branch(children, _) | Self::NibbledBranch(_, children, _) =>
				ChildIter::Array(children, 0),
		}
	}

	/// Returns the hash of the data attached to this node.
	pub fn data_hash(&self) -> Option<H> {
		match &self {
			Self::Empty => None,
			Self::Leaf(_, value) => value.data_hash(),
			Self::Extension(_, _) => None,
			Self::Branch(_, value) => value.as_ref().and_then(|v| v.data_hash()),
			Self::NibbledBranch(_, _, value) => value.as_ref().and_then(|v| v.data_hash()),
			Self::Value(_, hash) => Some(*hash),
		}
	}
}

impl<H> NodeOwned<H> {
	/// Returns the data attached to this node.
	pub fn data(&self) -> Option<&Bytes> {
		match &self {
			Self::Empty => None,
			Self::Leaf(_, value) => value.data(),
			Self::Extension(_, _) => None,
			Self::Branch(_, value) => value.as_ref().and_then(|v| v.data()),
			Self::NibbledBranch(_, _, value) => value.as_ref().and_then(|v| v.data()),
			Self::Value(data, _) => Some(data),
		}
	}

	/// Returns the partial key of this node.
	pub fn partial_key(&self) -> Option<&NibbleVec> {
		match self {
			Self::Branch(_, _) | Self::Value(_, _) | Self::Empty => None,
			Self::Extension(partial, _) |
			Self::Leaf(partial, _) |
			Self::NibbledBranch(partial, _, _) => Some(partial),
		}
	}

	/// Returns the size in bytes of this node in memory.
	///
	/// This also includes the size of any inline child nodes.
	pub fn size_in_bytes(&self) -> usize {
		let self_size = mem::size_of::<Self>();

		fn childs_size<'a, H: 'a>(
			childs: impl Iterator<Item = &'a Option<NodeHandleOwned<H>>>,
		) -> usize {
			// If a `child` isn't an inline node, its size is already taken account for by
			// `self_size`.
			childs
				.filter_map(|c| c.as_ref())
				.map(|c| c.as_inline().map_or(0, |n| n.size_in_bytes()))
				.sum()
		}

		// As `self_size` only represents the static size of `Self`, we also need
		// to add the size of any dynamically allocated data.
		let dynamic_size = match self {
			Self::Empty => 0,
			Self::Leaf(nibbles, value) =>
				nibbles.inner().len() + value.data().map_or(0, |b| b.len()),
			Self::Value(bytes, _) => bytes.len(),
			Self::Extension(nibbles, child) => {
				// If the `child` isn't an inline node, its size is already taken account for by
				// `self_size`.
				nibbles.inner().len() + child.as_inline().map_or(0, |n| n.size_in_bytes())
			},
			Self::Branch(childs, value) =>
				childs_size(childs.iter()) +
					value.as_ref().and_then(|v| v.data()).map_or(0, |b| b.len()),
			Self::NibbledBranch(nibbles, childs, value) =>
				nibbles.inner().len() +
					childs_size(childs.iter()) +
					value.as_ref().and_then(|v| v.data()).map_or(0, |b| b.len()),
		};

		self_size + dynamic_size
	}
}

/// A `NodeHandlePlan` is a decoding plan for constructing a `NodeHandle` from an encoded trie
/// node. This is used as a substructure of `NodePlan`. See `NodePlan` for details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeHandlePlan {
	Hash(Range<usize>),
	Inline(Range<usize>),
}

impl NodeHandlePlan {
	/// Build a node handle by decoding a byte slice according to the node handle plan. It is the
	/// responsibility of the caller to ensure that the node plan was created for the argument
	/// data, otherwise the call may decode incorrectly or panic.
	pub fn build<'a, 'b>(&'a self, data: &'b [u8]) -> NodeHandle<'b> {
		match self {
			NodeHandlePlan::Hash(range) => NodeHandle::Hash(&data[range.clone()]),
			NodeHandlePlan::Inline(range) => NodeHandle::Inline(&data[range.clone()]),
		}
	}
}

/// A `NibbleSlicePlan` is a blueprint for decoding a nibble slice from a byte slice. The
/// `NibbleSlicePlan` is created by parsing a byte slice and can be reused multiple times.
#[derive(Eq, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct NibbleSlicePlan {
	bytes: Range<usize>,
	offset: usize,
}

impl NibbleSlicePlan {
	/// Construct a nibble slice decode plan.
	pub fn new(bytes: Range<usize>, offset: usize) -> Self {
		NibbleSlicePlan { bytes, offset }
	}

	/// Returns the nibble length of the slice.
	pub fn len(&self) -> usize {
		(self.bytes.end - self.bytes.start) * nibble_ops::NIBBLE_PER_BYTE - self.offset
	}

	/// Build a nibble slice by decoding a byte slice according to the plan. It is the
	/// responsibility of the caller to ensure that the node plan was created for the argument
	/// data, otherwise the call may decode incorrectly or panic.
	pub fn build<'a, 'b>(&'a self, data: &'b [u8]) -> NibbleSlice<'b> {
		NibbleSlice::new_offset(&data[self.bytes.clone()], self.offset)
	}
}

/// Plan for value representation in `NodePlan`.
#[derive(Eq, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum ValuePlan {
	/// Range for byte representation in encoded node.
	Inline(Range<usize>),
	/// Range for hash in encoded node and original
	/// value size.
	Node(Range<usize>),
}

impl ValuePlan {
	/// Build a value slice by decoding a byte slice according to the plan.
	pub fn build<'a, 'b>(&'a self, data: &'b [u8]) -> Value<'b> {
		match self {
			ValuePlan::Inline(range) => Value::Inline(&data[range.clone()]),
			ValuePlan::Node(range) => Value::Node(&data[range.clone()]),
		}
	}
}

/// A `NodePlan` is a blueprint for decoding a node from a byte slice. The `NodePlan` is created
/// by parsing an encoded node and can be reused multiple times. This is useful as a `Node` borrows
/// from a byte slice and this struct does not.
///
/// The enum values mirror those of `Node` except that instead of byte slices, this struct stores
/// ranges that can be used to index into a large byte slice.
#[derive(Eq, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum NodePlan {
	/// Null trie node; could be an empty root or an empty branch entry.
	Empty,
	/// Leaf node; has a partial key plan and value.
	Leaf { partial: NibbleSlicePlan, value: ValuePlan },
	/// Extension node; has a partial key plan and child data.
	Extension { partial: NibbleSlicePlan, child: NodeHandlePlan },
	/// Branch node; has slice of child nodes (each possibly null)
	/// and an optional immediate node data.
	Branch {
		value: Option<ValuePlan>,
		children: [Option<NodeHandlePlan>; nibble_ops::NIBBLE_LENGTH],
	},
	/// Branch node with support for a nibble (when extension nodes are not used).
	NibbledBranch {
		partial: NibbleSlicePlan,
		value: Option<ValuePlan>,
		children: [Option<NodeHandlePlan>; nibble_ops::NIBBLE_LENGTH],
	},
}

impl NodePlan {
	/// Build a node by decoding a byte slice according to the node plan. It is the responsibility
	/// of the caller to ensure that the node plan was created for the argument data, otherwise the
	/// call may decode incorrectly or panic.
	pub fn build<'a, 'b>(&'a self, data: &'b [u8]) -> Node<'b> {
		match self {
			NodePlan::Empty => Node::Empty,
			NodePlan::Leaf { partial, value } => Node::Leaf(partial.build(data), value.build(data)),
			NodePlan::Extension { partial, child } =>
				Node::Extension(partial.build(data), child.build(data)),
			NodePlan::Branch { value, children } => {
				let mut child_slices = [None; nibble_ops::NIBBLE_LENGTH];
				for i in 0..nibble_ops::NIBBLE_LENGTH {
					child_slices[i] = children[i].as_ref().map(|child| child.build(data));
				}
				Node::Branch(child_slices, value.as_ref().map(|v| v.build(data)))
			},
			NodePlan::NibbledBranch { partial, value, children } => {
				let mut child_slices = [None; nibble_ops::NIBBLE_LENGTH];
				for i in 0..nibble_ops::NIBBLE_LENGTH {
					child_slices[i] = children[i].as_ref().map(|child| child.build(data));
				}
				Node::NibbledBranch(
					partial.build(data),
					child_slices,
					value.as_ref().map(|v| v.build(data)),
				)
			},
		}
	}

	/// Access value plan from node plan, return `None` for
	/// node that cannot contain a `ValuePlan`.
	pub fn value_plan(&self) -> Option<&ValuePlan> {
		match self {
			NodePlan::Extension { .. } | NodePlan::Empty => None,
			NodePlan::Leaf { value, .. } => Some(value),
			NodePlan::Branch { value, .. } | NodePlan::NibbledBranch { value, .. } =>
				value.as_ref(),
		}
	}

	/// Mutable ccess value plan from node plan, return `None` for
	/// node that cannot contain a `ValuePlan`.
	pub fn value_plan_mut(&mut self) -> Option<&mut ValuePlan> {
		match self {
			NodePlan::Extension { .. } | NodePlan::Empty => None,
			NodePlan::Leaf { value, .. } => Some(value),
			NodePlan::Branch { value, .. } | NodePlan::NibbledBranch { value, .. } =>
				value.as_mut(),
		}
	}
}

/// An `OwnedNode` is an owned type from which a `Node` can be constructed which borrows data from
/// the `OwnedNode`. This is useful for trie iterators.
#[cfg_attr(feature = "std", derive(Debug))]
#[derive(PartialEq, Eq)]
pub struct OwnedNode<D: Borrow<[u8]>> {
	data: D,
	plan: NodePlan,
}

impl<D: Borrow<[u8]>> OwnedNode<D> {
	/// Construct an `OwnedNode` by decoding an owned data source according to some codec.
	pub fn new<C: NodeCodec>(data: D) -> core::result::Result<Self, C::Error> {
		let plan = C::decode_plan(data.borrow())?;
		Ok(OwnedNode { data, plan })
	}

	/// Returns a reference to the backing data.
	pub fn data(&self) -> &[u8] {
		self.data.borrow()
	}

	/// Returns a reference to the node decode plan.
	pub fn node_plan(&self) -> &NodePlan {
		&self.plan
	}

	/// Returns a mutable reference to the node decode plan.
	pub fn node_plan_mut(&mut self) -> &mut NodePlan {
		&mut self.plan
	}

	/// Construct a `Node` by borrowing data from this struct.
	pub fn node(&self) -> Node {
		self.plan.build(self.data.borrow())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{nibble_ops, ChildReference, TrieLayout};
	use core::{cmp::Ordering, convert::Infallible};
	use hash_db::Hasher;
	use std::collections::hash_map::DefaultHasher;

	#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
	struct TestHash([u8; 32]);

	impl AsRef<[u8]> for TestHash {
		fn as_ref(&self) -> &[u8] {
			&self.0
		}
	}

	impl AsMut<[u8]> for TestHash {
		fn as_mut(&mut self) -> &mut [u8] {
			&mut self.0
		}
	}

	impl Ord for TestHash {
		fn cmp(&self, other: &Self) -> Ordering {
			self.0.cmp(&other.0)
		}
	}

	impl PartialOrd for TestHash {
		fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
			Some(self.cmp(other))
		}
	}

	struct TestHasher;

	impl Hasher for TestHasher {
		type Out = TestHash;
		type StdHasher = DefaultHasher;
		const LENGTH: usize = 32;

		fn hash(x: &[u8]) -> Self::Out {
			let mut out = [0u8; 32];
			let to_copy = core::cmp::min(out.len(), x.len());
			out[..to_copy].copy_from_slice(&x[..to_copy]);
			TestHash(out)
		}
	}

	struct TestCodec;

	impl crate::NodeCodec for TestCodec {
		type Error = Infallible;
		type HashOut = TestHash;

		fn hashed_null_node() -> Self::HashOut {
			TestHash([0u8; 32])
		}

		fn decode_plan(_data: &[u8]) -> core::result::Result<NodePlan, Self::Error> {
			panic!("decode_plan is not used in these tests")
		}

		fn is_empty_node(_data: &[u8]) -> bool {
			false
		}

		fn empty_node() -> &'static [u8] {
			&[]
		}

		fn leaf_node(
			_partial: impl Iterator<Item = u8>,
			_number_nibble: usize,
			_value: Value,
		) -> Vec<u8> {
			panic!("leaf_node is not used in these tests")
		}

		fn extension_node(
			_partial: impl Iterator<Item = u8>,
			_number_nibble: usize,
			_child_ref: ChildReference<Self::HashOut>,
		) -> Vec<u8> {
			panic!("extension_node is not used in these tests")
		}

		fn branch_node(
			_children: impl Iterator<Item = impl Borrow<Option<ChildReference<Self::HashOut>>>>,
			_value: Option<Value>,
		) -> Vec<u8> {
			panic!("branch_node is not used in these tests")
		}

		fn branch_node_nibbled(
			_partial: impl Iterator<Item = u8>,
			_number_nibble: usize,
			_children: impl Iterator<Item = impl Borrow<Option<ChildReference<Self::HashOut>>>>,
			_value: Option<Value>,
		) -> Vec<u8> {
			panic!("branch_node_nibbled is not used in these tests")
		}
	}

	struct TestLayout;

	impl TrieLayout for TestLayout {
		const USE_EXTENSION: bool = true;
		const MAX_INLINE_VALUE: Option<u32> = Some(32);
		type Hash = TestHasher;
		type Codec = TestCodec;
	}

	#[test]
	fn branch_to_owned_node_preserves_hash_children() {
		let h0 = [0x11u8; 32];
		let h15 = [0x22u8; 32];
		let mut children = [(); nibble_ops::NIBBLE_LENGTH].map(|_| None);
		children[0] = Some(NodeHandle::Hash(&h0));
		children[15] = Some(NodeHandle::Hash(&h15));

		let owned = Node::Branch(children, None)
			.to_owned_node::<TestLayout>()
			.expect("branch conversion should succeed");
		let NodeOwned::Branch(converted, None) = owned else {
			panic!("expected converted branch node");
		};

		assert!(matches!(
			converted[0],
			Some(NodeHandleOwned::Hash(hash)) if hash.0 == h0
		));
		assert!(matches!(
			converted[15],
			Some(NodeHandleOwned::Hash(hash)) if hash.0 == h15
		));
	}

	#[test]
	fn nibbled_branch_to_owned_node_preserves_hash_children() {
		let h3 = [0x33u8; 32];
		let mut children = [(); nibble_ops::NIBBLE_LENGTH].map(|_| None);
		children[3] = Some(NodeHandle::Hash(&h3));
		let partial = NibbleSlice::new(&[0xabu8, 0xcdu8]);

		let owned = Node::NibbledBranch(partial, children, None)
			.to_owned_node::<TestLayout>()
			.expect("nibbled branch conversion should succeed");
		let NodeOwned::NibbledBranch(_, converted, None) = owned else {
			panic!("expected converted nibbled branch node");
		};

		assert!(matches!(
			converted[3],
			Some(NodeHandleOwned::Hash(hash)) if hash.0 == h3
		));
	}

	#[test]
	fn branch_to_owned_node_rejects_invalid_hash_child_length() {
		let mut children = [(); nibble_ops::NIBBLE_LENGTH].map(|_| None);
		children[7] = Some(NodeHandle::Hash(&[0x11u8; 31]));

		assert!(Node::Branch(children, None).to_owned_node::<TestLayout>().is_err());
	}

	#[test]
	fn nibbled_branch_to_owned_node_rejects_invalid_hash_child_length() {
		let mut children = [(); nibble_ops::NIBBLE_LENGTH].map(|_| None);
		children[2] = Some(NodeHandle::Hash(&[0x22u8; 33]));
		let partial = NibbleSlice::new(&[0x01u8, 0x23u8]);

		assert!(
			Node::NibbledBranch(partial, children, None)
				.to_owned_node::<TestLayout>()
				.is_err()
		);
	}
}
