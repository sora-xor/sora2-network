use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use ethereum_types::{H128, H512};
use sha2::Digest;

#[derive(Clone, Copy)]
pub struct ElementData([u8; 128]);

impl ElementData {
    pub fn to_h512_pair(&self) -> [H512; 2] {
        let data = self.conventional();
        [
            H512::from_slice(&data.0[..64]),
            H512::from_slice(&data.0[64..]),
        ]
    }

    fn conventional(mut self) -> Self {
        self.as_mut()[..32].reverse();
        self.as_mut()[32..64].reverse();
        self.as_mut()[64..96].reverse();
        self.as_mut()[96..].reverse();
        self
    }

    fn hash(&self) -> H128 {
        let data = self.clone().conventional();
        H128::from_slice(&sha2::Sha256::digest(data).as_slice()[16..])
    }
}

impl Debug for ElementData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ElementData")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

impl Default for ElementData {
    fn default() -> Self {
        ElementData([0; 128])
    }
}

impl From<[u8; 128]> for ElementData {
    fn from(v: [u8; 128]) -> Self {
        ElementData(v)
    }
}

impl AsRef<[u8]> for ElementData {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for ElementData {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

fn sha256_hash(a: H128, b: H128) -> H128 {
    let mut data = H512::zero();
    data.as_fixed_bytes_mut()[16..32].copy_from_slice(a.as_bytes());
    data.as_fixed_bytes_mut()[48..].copy_from_slice(b.as_bytes());
    let res = sha2::Sha256::digest(data.as_bytes());
    let res = H128::from_slice(&res.as_slice()[16..]);
    res
}

#[derive(Clone, Default, Debug)]
pub struct BranchTree {
    data: ElementData,
    root: BranchNode,
}

impl BranchTree {
    fn collect_nodes(&self) -> Vec<H128> {
        self.root.collect_nodes()
    }
}

#[derive(Clone, Default, Debug)]
pub struct BranchNode {
    hash: H128,
    left: Option<Box<BranchNode>>,
    right: Option<Box<BranchNode>>,
    left_element: bool,
}

impl BranchNode {
    fn collect_nodes(&self) -> Vec<H128> {
        match (&self.left, &self.right) {
            (None, None) => vec![self.hash],
            (Some(left), Some(right)) => {
                let mut left = left.collect_nodes();
                let mut right = right.collect_nodes();
                if self.left_element {
                    left.extend(right);
                    left
                } else {
                    right.extend(left);
                    right
                }
            }
            _ => unreachable!(),
        }
    }
}

impl BranchNode {
    fn accept_right_sibling(&mut self, hash: H128) {
        let b = std::mem::replace(self, BranchNode::default());
        *self = BranchNode {
            left_element: true,
            left: Some(Box::new(b)),
            right: Some(Box::new(BranchNode {
                hash,
                ..Default::default()
            })),
            ..Default::default()
        };
    }

    fn accept_left_sibling(&mut self, hash: H128) {
        let b = std::mem::replace(self, BranchNode::default());
        *self = BranchNode {
            right: Some(Box::new(b)),
            left: Some(Box::new(BranchNode {
                hash,
                ..Default::default()
            })),
            ..Default::default()
        };
    }
}

#[derive(Clone, Default, Debug)]
struct Node {
    hash: H128,
    count: u32,
    branches: HashMap<u32, BranchTree>,
}

#[derive(Clone, Default, Debug)]
pub struct MerkleTree {
    buf: Vec<Node>,
    indexes: HashSet<u32>,
    ordered_indexes: Vec<u32>,
    finalized: bool,
}

impl MerkleTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_index(&mut self, indexes: Vec<u32>) {
        for index in indexes {
            self.indexes.insert(index);
            self.ordered_indexes.push(index);
        }
    }

    pub fn branches(&self) -> &HashMap<u32, BranchTree> {
        if self.finalized {
            &self.buf.first().unwrap().branches
        } else {
            panic!("not finalized tree");
        }
    }

    pub fn indexes(&self) -> &[u32] {
        &self.ordered_indexes
    }

    pub fn proofs_for_ordered_indexes(&self) -> Vec<Vec<H128>> {
        let mut res = vec![];
        let branches = self.branches();
        for index in self.indexes() {
            let mut proof = vec![];
            let hh = branches.get(index).unwrap().collect_nodes();
            let hashes = &hh[1..];
            for hash in hashes.iter() {
                proof.push(hash.clone());
            }
            res.push(proof)
        }
        res
    }

    pub fn first_element(&self) -> ElementData {
        self.branches().get(&self.indexes()[0]).unwrap().data
    }

    pub fn insert(&mut self, data: ElementData, index: u32) {
        let mut node = Node {
            hash: data.hash(),
            count: 1,
            branches: Default::default(),
        };
        if self.indexes.contains(&index) {
            node.branches.insert(
                index,
                BranchTree {
                    data,
                    root: BranchNode {
                        hash: node.hash.clone(),
                        ..Default::default()
                    },
                },
            );
        }
        self.insert_node(node);
    }

    fn insert_node(&mut self, node: Node) {
        self.buf.push(node);
        loop {
            if self.buf.len() < 2 {
                return;
            }
            let cur = self.buf.pop().unwrap();
            let mut prev = self.buf.pop().unwrap();
            if cur.count != prev.count {
                self.buf.push(prev);
                self.buf.push(cur);
                return;
            }

            for v in prev.branches.values_mut() {
                v.root.accept_right_sibling(cur.hash);
            }

            for (k, mut v) in cur.branches.into_iter() {
                v.root.accept_left_sibling(prev.hash);
                prev.branches.insert(k, v);
            }

            prev.hash = sha256_hash(prev.hash, cur.hash);
            prev.count = cur.count * 2 + 1;

            self.buf.push(prev);
        }
    }

    pub fn finalize(&mut self) {
        if !self.finalized && self.buf.len() > 1 {
            while self.buf.len() > 1 {
                let node = self.buf.last().unwrap().clone();
                self.insert_node(node);
            }
        }
        self.finalized = true;
    }

    pub fn root(&self) -> H128 {
        if self.finalized {
            self.buf.first().unwrap().hash
        } else {
            panic!("not finalized tree");
        }
    }
}
