// Copyright (C) 2022-2025 Parity Technologies (UK) Ltd. (admin@parity.io)
// This file is a part of the frame-decode crate.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//         http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use alloc::vec::Vec;
use scale_type_resolver::TypeResolver;

/// This can be implemented for anything that can be encoded in multiple steps into a set of values
/// via [`scale_encode::EncodeAsType`]. The common use case is to encode a tuple of multiple types,
/// step by step, into bytes. As well as tuples up to size 12, Implementations also exist for Vecs
/// and arrays.
pub trait IntoEncodableValues {
    /// An implementation of [`EncodableValues`] that can be used to iterate through the values.
    type Values<'this>: EncodableValues
    where
        Self: 'this;
    /// Return an implementation of [`EncodableValues`] for this type.
    // Dev note: for naming consistency with IntoDecodableValues we call this method
    // into_encodable_values, but it doesn't need to take ownership of self, which
    // clippy doesn't like, hence the allow.
    #[allow(clippy::wrong_self_convention)]
    fn into_encodable_values(&self) -> Self::Values<'_>;
    /// The number of values that can be encoded from this type.
    fn num_encodable_values(&self) -> usize;
}

/// Since [`scale_encode::EncodeAsType`] is not dyn safe, this trait is used to iterate through and
/// encode a set of values.
pub trait EncodableValues {
    /// Encode the next value, if there is one, into the provided output buffer. This method
    /// must not be called more times than [`IntoEncodableValues::num_encodable_values`].
    ///
    /// # Panics
    ///
    /// This method may panic if we call it more than [`IntoEncodableValues::num_encodable_values`]
    /// times (ie we try to encode more values than actually exist).
    fn encode_next_value_to<Resolver>(
        &mut self,
        type_id: Resolver::TypeId,
        types: &Resolver,
        out: &mut Vec<u8>,
    ) -> Result<(), scale_encode::Error>
    where
        Resolver: TypeResolver;

    /// Encode the next value, if there is one, and return the encoded bytes. This method
    /// must not be called more times than [`IntoEncodableValues::num_encodable_values`].
    ///
    /// # Panics
    ///
    /// This method may panic if we call it more than [`IntoEncodableValues::num_encodable_values`]
    /// times (ie we try to encode more values than actually exist).
    fn encode_next_value<Resolver>(
        &mut self,
        type_id: Resolver::TypeId,
        types: &Resolver,
    ) -> Result<Vec<u8>, scale_encode::Error>
    where
        Resolver: TypeResolver,
    {
        let mut out = Vec::new();
        self.encode_next_value_to(type_id, types, &mut out)
            .map(|_| out)
    }
}

// References are ok
impl<'a, T: IntoEncodableValues> IntoEncodableValues for &'a T {
    type Values<'this>
        = T::Values<'this>
    where
        'a: 'this;

    fn into_encodable_values(&self) -> Self::Values<'_> {
        (*self).into_encodable_values()
    }
    fn num_encodable_values(&self) -> usize {
        (*self).num_encodable_values()
    }
}

// Vecs
impl<K: scale_encode::EncodeAsType> IntoEncodableValues for Vec<K> {
    type Values<'this>
        = core::slice::Iter<'this, K>
    where
        K: 'this;
    fn num_encodable_values(&self) -> usize {
        self.len()
    }
    fn into_encodable_values(&self) -> Self::Values<'_> {
        self.iter()
    }
}

impl<'a, K: scale_encode::EncodeAsType> EncodableValues for core::slice::Iter<'a, K> {
    fn encode_next_value_to<Resolver>(
        &mut self,
        type_id: Resolver::TypeId,
        types: &Resolver,
        out: &mut Vec<u8>,
    ) -> Result<(), scale_encode::Error>
    where
        Resolver: TypeResolver,
    {
        let Some(next_key) = self.next() else {
            return Err(scale_encode::Error::custom_str(
                "encode_next_value_to called but no more values to encode",
            ));
        };
        next_key.encode_as_type_to(type_id, types, out)
    }
}

// Arrays (uses the same iter as above)
impl<K: scale_encode::EncodeAsType, const N: usize> IntoEncodableValues for [K; N] {
    type Values<'this>
        = core::slice::Iter<'this, K>
    where
        K: 'this;
    fn num_encodable_values(&self) -> usize {
        N
    }
    fn into_encodable_values(&self) -> Self::Values<'_> {
        self.iter()
    }
}

// Empty tuples can be used as a placeholder for no values.
impl IntoEncodableValues for () {
    type Values<'this> = ();
    fn num_encodable_values(&self) -> usize {
        0
    }
    fn into_encodable_values(&self) -> Self::Values<'_> {}
}

impl EncodableValues for () {
    fn encode_next_value_to<Resolver>(
        &mut self,
        _type_id: Resolver::TypeId,
        _types: &Resolver,
        _out: &mut Vec<u8>,
    ) -> Result<(), scale_encode::Error>
    where
        Resolver: TypeResolver,
    {
        Err(scale_encode::Error::custom_str(
            "encode_next_value_to called on an empty tuple",
        ))
    }
}

// Tuples of different lengths can be encoded as values too.
macro_rules! impl_tuple_encodable {
    ($($ty:ident $number:tt),*) => {
        const _: () = {
            const TUPLE_LEN: usize = 0 $(+ $number - $number + 1)*;
            type TupleOf<$($ty),*> = ($($ty,)*);

            impl <$($ty: scale_encode::EncodeAsType),*> IntoEncodableValues for TupleOf<$($ty),*> {
                type Values<'this> = TupleIter<'this, TupleOf<$($ty),*>> where TupleOf<$($ty),*>: 'this;
                fn num_encodable_values(&self) -> usize {
                    TUPLE_LEN
                }
                fn into_encodable_values(&self) -> Self::Values<'_> {
                    TupleIter {
                        idx: 0,
                        items: self,
                    }
                }
            }

            pub struct TupleIter<'this, TupleTy> {
                idx: usize,
                items: &'this TupleTy
            }

            impl <'a, $($ty: scale_encode::EncodeAsType),*> EncodableValues for TupleIter<'a, TupleOf<$($ty),*>> {
                fn encode_next_value_to<Resolver>(&mut self, type_id: Resolver::TypeId, types: &Resolver, out: &mut Vec<u8>) -> Result<(), scale_encode::Error>
                where
                    Resolver: TypeResolver,
                {
                    $(
                        if self.idx == $number {
                            let item = &self.items.$number;
                            item.encode_as_type_to(type_id, types, out)?;
                            self.idx += 1;
                            return Ok(());
                        }
                    )*
                    Err(scale_encode::Error::custom_str("encode_next_value_to called but no more tuple entries to encode"))
                }
            }
        };
    };
}

impl_tuple_encodable!(A 0);
impl_tuple_encodable!(A 0, B 1);
impl_tuple_encodable!(A 0, B 1, C 2);
impl_tuple_encodable!(A 0, B 1, C 2, D 3);
impl_tuple_encodable!(A 0, B 1, C 2, D 3, E 4);
impl_tuple_encodable!(A 0, B 1, C 2, D 3, E 4, F 5);
impl_tuple_encodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
impl_tuple_encodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
impl_tuple_encodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8);
impl_tuple_encodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9);
impl_tuple_encodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10);
impl_tuple_encodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11);

#[cfg(test)]
mod test {
    use super::*;
    use parity_scale_codec::Encode;
    use scale_info_legacy::LookupName;

    fn ln(ty: &str) -> LookupName {
        LookupName::parse(ty).unwrap()
    }

    #[test]
    fn test_tuple_encodable_values() {
        // We just need some basic types to test with.
        let types = crate::legacy_types::polkadot::relay_chain();
        let types = types.for_spec_version(0);

        let keys = (123u16, true, "hello");
        assert_eq!(keys.num_encodable_values(), 3);
        let mut encodable_values = keys.into_encodable_values();

        let val = encodable_values
            .encode_next_value(ln("u64"), &types)
            .unwrap();
        assert_eq!(val, 123u64.encode());

        let val = encodable_values
            .encode_next_value(ln("bool"), &types)
            .unwrap();
        assert_eq!(val, true.encode());

        let val = encodable_values
            .encode_next_value(ln("String"), &types)
            .unwrap();
        assert_eq!(val, "hello".encode());

        // These _could_ panic in theory but our impls don't.
        assert!(
            encodable_values
                .encode_next_value(ln("foo"), &types)
                .is_err()
        );
        assert!(
            encodable_values
                .encode_next_value(ln("foo"), &types)
                .is_err()
        );
    }

    #[test]
    fn test_vec_encodable_values() {
        // We just need some basic types to test with.
        let types = crate::legacy_types::polkadot::relay_chain();
        let types = types.for_spec_version(0);

        let keys = vec![123u16, 456u16, 789u16];
        assert_eq!(keys.num_encodable_values(), 3);
        let mut encodable_values = keys.into_encodable_values();

        let val = encodable_values
            .encode_next_value(ln("u64"), &types)
            .unwrap();
        assert_eq!(val, 123u64.encode());

        let val = encodable_values
            .encode_next_value(ln("u16"), &types)
            .unwrap();
        assert_eq!(val, 456u16.encode());

        let val = encodable_values
            .encode_next_value(ln("u32"), &types)
            .unwrap();
        assert_eq!(val, 789u32.encode());

        // These _could_ panic in theory but our impls don't.
        assert!(
            encodable_values
                .encode_next_value(ln("foo"), &types)
                .is_err()
        );
        assert!(
            encodable_values
                .encode_next_value(ln("foo"), &types)
                .is_err()
        );
    }

    #[test]
    fn test_array_encodable_values() {
        // We just need some basic types to test with.
        let types = crate::legacy_types::polkadot::relay_chain();
        let types = types.for_spec_version(0);

        let keys: [u16; 3] = [123, 456, 789];
        assert_eq!(keys.num_encodable_values(), 3);
        let mut encodable_values = keys.into_encodable_values();

        let val = encodable_values
            .encode_next_value(ln("u64"), &types)
            .unwrap();
        assert_eq!(val, 123u64.encode());

        let val = encodable_values
            .encode_next_value(ln("u16"), &types)
            .unwrap();
        assert_eq!(val, 456u16.encode());

        let val = encodable_values
            .encode_next_value(ln("u32"), &types)
            .unwrap();
        assert_eq!(val, 789u32.encode());

        // These _could_ panic in theory but our impls don't.
        assert!(
            encodable_values
                .encode_next_value(ln("foo"), &types)
                .is_err()
        );
        assert!(
            encodable_values
                .encode_next_value(ln("foo"), &types)
                .is_err()
        );
    }
}
