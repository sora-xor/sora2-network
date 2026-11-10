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

use alloc::format;
use alloc::vec::Vec;
use scale_decode::DecodeAsType;
use scale_type_resolver::TypeResolver;

/// This can be implemented for any type that can be decoded into in multiple steps via
/// [`scale_decode::DecodeAsType`]. The common use case is to decode some sets of bytes into a
/// tuple of multiple types, step by step. As well as tuples up to size 12, Implementations also exist
/// and arrays.
pub trait IntoDecodableValues: Sized {
    /// The decoder we'll use to iteratively decode bytes.
    type Values: DecodableValues<Target = Self>;

    /// Construct a type that is ready to decode values
    /// and return the target type.
    fn into_decodable_values() -> Self::Values;

    /// The exact number of values that should be provided to
    /// [`DecodableValues::decode_next_value()`] before
    /// [`DecodableValues::decoded_target()`] can be called. If this
    /// returns `None` then it indicates that any number of values
    /// can be decoded into `Self`.
    fn num_decodable_values() -> Option<usize>;
}

/// Implementors of this trait are capable of decoding multiple values from bytes into an output type
/// such as a tuple or fixed size array.
pub trait DecodableValues {
    /// The target that we will return after decoding multiple values.
    type Target;

    /// Decode the next value. This should be called exactly as
    /// many times as [`IntoDecodableValues::num_decodable_values`], after
    /// which [`DecodableValues::decoded_target()`] can be called to
    /// return the decoded target value.
    ///
    /// # Panics
    ///
    /// This method may panic if it is called more times
    /// than [`IntoDecodableValues::num_decodable_values`].
    fn decode_next_value<Resolver>(
        &mut self,
        input: &mut &[u8],
        type_id: Resolver::TypeId,
        types: &Resolver,
    ) -> Result<(), scale_decode::Error>
    where
        Resolver: TypeResolver;

    /// Return the decoded target.
    ///
    /// # Panics
    ///
    /// This method may panic if [`DecodableValues::decode_next_value`] has
    /// not been called enough times.
    fn decoded_target(self) -> Self::Target;
}

// Vecs
impl<T: DecodeAsType> IntoDecodableValues for Vec<T> {
    type Values = Self;
    fn into_decodable_values() -> Self::Values {
        Vec::new()
    }
    fn num_decodable_values() -> Option<usize> {
        None
    }
}

impl<T: DecodeAsType> DecodableValues for Vec<T> {
    type Target = Self;

    fn decode_next_value<Resolver>(
        &mut self,
        input: &mut &[u8],
        type_id: Resolver::TypeId,
        types: &Resolver,
    ) -> Result<(), scale_decode::Error>
    where
        Resolver: TypeResolver,
    {
        let item = T::decode_as_type(input, type_id, types)?;
        self.push(item);
        Ok(())
    }

    fn decoded_target(self) -> Self::Target {
        self
    }
}

// Arrays
impl<const N: usize, T: DecodeAsType> IntoDecodableValues for [T; N] {
    type Values = DecodeArrayAsTypes<N, T>;
    fn into_decodable_values() -> Self::Values {
        DecodeArrayAsTypes {
            items: [const { None }; N],
            next_idx: 0,
        }
    }
    fn num_decodable_values() -> Option<usize> {
        Some(N)
    }
}

pub struct DecodeArrayAsTypes<const N: usize, T> {
    items: [Option<T>; N],
    next_idx: usize,
}

impl<const N: usize, T: DecodeAsType> DecodableValues for DecodeArrayAsTypes<N, T> {
    type Target = [T; N];

    fn decode_next_value<Resolver>(
        &mut self,
        input: &mut &[u8],
        type_id: Resolver::TypeId,
        types: &Resolver,
    ) -> Result<(), scale_decode::Error>
    where
        Resolver: TypeResolver,
    {
        if self.next_idx >= N {
            let e = format!(
                "decode_next_value called too many times (expected {N} calls) to decode [{}; N]",
                core::any::type_name::<T>()
            );
            return Err(scale_decode::Error::custom_string(e));
        }

        let item = T::decode_as_type(input, type_id, types)?;

        self.items[self.next_idx] = Some(item);
        self.next_idx += 1;
        Ok(())
    }
    fn decoded_target(self) -> Self::Target {
        if self.next_idx != N {
            panic!(
                "decode_next_value was not called enough times (expected {N} calls, got {} calls) to decode [{}; N]",
                self.next_idx,
                core::any::type_name::<T>()
            )
        }

        // This could be done slightly more efficiently with unsafe and MaybeUninit but not worth it.
        let mut items = self.items;
        core::array::from_fn(|idx| {
            items[idx]
                .take()
                .expect("Item should be present in DecodeArrayAsType array")
        })
    }
}

// Empty Tuples
impl IntoDecodableValues for () {
    type Values = ();
    fn into_decodable_values() -> Self::Values {}
    fn num_decodable_values() -> Option<usize> {
        Some(0)
    }
}

impl DecodableValues for () {
    type Target = ();

    fn decode_next_value<Resolver>(
        &mut self,
        _input: &mut &[u8],
        _type_id: Resolver::TypeId,
        _types: &Resolver,
    ) -> Result<(), scale_decode::Error>
    where
        Resolver: TypeResolver,
    {
        Err(scale_decode::Error::custom_str(
            "decode_next_value cannot be called on an empty tuple",
        ))
    }

    fn decoded_target(self) -> Self::Target {}
}

// Non-empty tuples
macro_rules! impl_tuple_decodable {
    ($($ty:ident $number:tt),*) => {
        const _: () = {
            const TUPLE_LEN: usize = 0 $(+ $number - $number + 1)*;

            impl <$($ty: scale_decode::DecodeAsType),*> IntoDecodableValues for ($($ty,)*) {
                type Values = TupleIter<$($ty),*>;
                fn into_decodable_values() -> Self::Values {
                    TupleIter {
                        idx: 0,
                        items: ($(Option::<$ty>::None,)*),
                    }
                }
                fn num_decodable_values() -> Option<usize> {
                    Some(TUPLE_LEN)
                }
            }

            pub struct TupleIter<$($ty),*> {
                idx: usize,
                items: ($(Option<$ty>,)*)
            }

            impl <$($ty: scale_decode::DecodeAsType),*> DecodableValues for TupleIter<$($ty),*> {
                type Target = ($($ty,)*);

                fn decode_next_value<Resolver>(
                    &mut self,
                    input: &mut &[u8],
                    type_id: Resolver::TypeId,
                    types: &Resolver,
                ) -> Result<(), scale_decode::Error>
                where
                    Resolver: TypeResolver,
                {
                    $(
                        if self.idx == $number {
                            let item = $ty::decode_as_type(input, type_id, types)?;
                            self.items.$number = Some(item);
                            self.idx += 1;
                            return Ok(());
                        }
                    )*
                    Err(scale_decode::Error::custom_str("decode_next_value called but no more tuple entries to decode"))
                }
                fn decoded_target(self) -> Self::Target {
                    if self.idx != TUPLE_LEN {
                        panic!(
                            "decode_next_value not called enough times (expected {TUPLE_LEN} calls, got {} calls) to decode {}",
                            self.idx,
                            core::any::type_name::<Self::Target>()
                        )
                    }

                    (
                        $(
                          self.items.$number.unwrap(),
                        )*
                    )
                }
            }
        };
    };
}

impl_tuple_decodable!(A 0);
impl_tuple_decodable!(A 0, B 1);
impl_tuple_decodable!(A 0, B 1, C 2);
impl_tuple_decodable!(A 0, B 1, C 2, D 3);
impl_tuple_decodable!(A 0, B 1, C 2, D 3, E 4);
impl_tuple_decodable!(A 0, B 1, C 2, D 3, E 4, F 5);
impl_tuple_decodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
impl_tuple_decodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
impl_tuple_decodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8);
impl_tuple_decodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9);
impl_tuple_decodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10);
impl_tuple_decodable!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11);

#[cfg(test)]
mod test {
    use super::*;
    use parity_scale_codec::Encode;
    use scale_info_legacy::LookupName;

    fn ln(ty: &str) -> LookupName {
        LookupName::parse(ty).unwrap()
    }

    #[test]
    fn test_decode_empty_tuple() {
        // We just need some basic types to test with.
        let types = crate::legacy_types::polkadot::relay_chain();
        let types = types.for_spec_version(0);

        let n = <()>::num_decodable_values();
        assert_eq!(n, Some(0));

        // We could swap this with `()` but in theory we could change what was returned from
        // `into_decodable_values` and want to check that this would continue working.
        #[allow(clippy::let_unit_value)]
        let mut decodable = <()>::into_decodable_values();

        // Will error out immediately (the trait allows a panic here but our impls error)
        decodable
            .decode_next_value(&mut &*true.encode(), ln("bool"), &types)
            .unwrap_err();

        // This basically checks that the type of `.decoded_target()` is `()`:
        let () = decodable.decoded_target();
    }

    #[test]
    fn test_tuple_decodable_values() {
        // We just need some basic types to test with.
        let types = crate::legacy_types::polkadot::relay_chain();
        let types = types.for_spec_version(0);

        let n = <(bool, String, u64)>::num_decodable_values();
        assert_eq!(n, Some(3));

        let mut decodable = <(bool, String, u64)>::into_decodable_values();

        decodable
            .decode_next_value(&mut &*true.encode(), ln("bool"), &types)
            .unwrap();
        decodable
            .decode_next_value(&mut &*"hello".encode(), ln("String"), &types)
            .unwrap();
        decodable
            .decode_next_value(&mut &*123u8.encode(), ln("u8"), &types)
            .unwrap();

        // Will error out (the trait allows a panic here but our impls error)
        decodable
            .decode_next_value(&mut &*true.encode(), ln("bool"), &types)
            .unwrap_err();

        assert_eq!(
            decodable.decoded_target(),
            (true, String::from("hello"), 123u64)
        );
    }

    #[test]
    fn test_decode_empty_array() {
        // We just need some basic types to test with.
        let types = crate::legacy_types::polkadot::relay_chain();
        let types = types.for_spec_version(0);

        let n = <[u64; 0]>::num_decodable_values();
        assert_eq!(n, Some(0));

        let mut decodable = <[u64; 0]>::into_decodable_values();

        // Will error out (the trait allows a panic here but our impls error)
        decodable
            .decode_next_value(&mut &*1u32.encode(), ln("u32"), &types)
            .unwrap_err();

        assert_eq!(decodable.decoded_target(), [] as [u64; 0]);
    }

    #[test]
    fn test_decodable_array() {
        // We just need some basic types to test with.
        let types = crate::legacy_types::polkadot::relay_chain();
        let types = types.for_spec_version(0);

        let n = <[u64; 3]>::num_decodable_values();
        assert_eq!(n, Some(3));

        let mut decodable = <[u64; 3]>::into_decodable_values();

        decodable
            .decode_next_value(&mut &*1u8.encode(), ln("u8"), &types)
            .unwrap();
        decodable
            .decode_next_value(&mut &*2u16.encode(), ln("u16"), &types)
            .unwrap();
        decodable
            .decode_next_value(&mut &*3u32.encode(), ln("u32"), &types)
            .unwrap();

        // Will error out (the trait allows a panic here but our impls error)
        decodable
            .decode_next_value(&mut &*4u32.encode(), ln("u32"), &types)
            .unwrap_err();

        assert_eq!(decodable.decoded_target(), [1u64, 2u64, 3u64]);
    }
}
