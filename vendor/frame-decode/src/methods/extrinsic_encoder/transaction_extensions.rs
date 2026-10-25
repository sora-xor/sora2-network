// Copyright (C) 2022-2026 Parity Technologies (UK) Ltd. (admin@parity.io)
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

use super::transaction_extension::{TransactionExtension, TransactionExtensionError};
use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use scale_type_resolver::TypeResolver;

/// This trait can be implemented for anything which represents a set of transaction extensions.
/// It's implemented by default for tuples of items which implement [`TransactionExtension`].
pub trait TransactionExtensions<Resolver: TypeResolver> {
    /// Is a given transaction extension contained within this set?
    fn contains_extension(&self, name: &str) -> bool;

    /// Does the named transaction extension authorize the transaction
    /// (see [`TransactionExtension::is_authorization_extension`])? This should
    /// return `false` for any extension not contained within this set.
    fn is_authorization_extension(&self, _name: &str) -> bool {
        false
    }

    /// This will be called given the name of each transaction extension we
    /// wish to obtain the encoded bytes to. Implementations are expected to
    /// write the bytes that should be included in the **transaction** to the given [`Vec`],
    /// or return an error if no such bytes can be written.
    fn encode_extension_value_to(
        &self,
        name: &str,
        type_id: Resolver::TypeId,
        type_resolver: &Resolver,
        out: &mut Vec<u8>,
    ) -> Result<(), TransactionExtensionsError>;

    /// This will be called given the name of each transaction extension we
    /// wish to obtain the encoded bytes to. Implementations are expected to
    /// write the bytes that should be included in the **signer payload implicit**
    /// to the given [`Vec`], or return an error if no such bytes can be written.
    fn encode_extension_implicit_to(
        &self,
        name: &str,
        type_id: Resolver::TypeId,
        type_resolver: &Resolver,
        out: &mut Vec<u8>,
    ) -> Result<(), TransactionExtensionsError>;
}

/// This error will be returned if any of the methods in [`TransactionExtensions`] fail.
#[derive(Debug, thiserror::Error)]
pub enum TransactionExtensionsError {
    /// The requested transaction extension could not be found.
    #[error("Cannot encode transaction extension '{0}': This extension could not be found")]
    NotFound(String),
    /// An error occurred while encoding the transaction extension.
    #[error("Cannot encode transaction extension '{extension_name}': {error}")]
    Other {
        /// The name of the extension that failed to encode.
        extension_name: String,
        /// The underlying error.
        error: TransactionExtensionError,
    },
}

// Empty tuples impl `TransactionExtensions`: if called they emit a not found error.
impl<Resolver: TypeResolver> TransactionExtensions<Resolver> for () {
    fn contains_extension(&self, _name: &str) -> bool {
        false
    }

    fn encode_extension_value_to(
        &self,
        name: &str,
        _type_id: <Resolver as TypeResolver>::TypeId,
        _type_resolver: &Resolver,
        _out: &mut Vec<u8>,
    ) -> Result<(), TransactionExtensionsError> {
        Err(TransactionExtensionsError::NotFound(name.to_owned()))
    }

    fn encode_extension_implicit_to(
        &self,
        name: &str,
        _type_id: <Resolver as TypeResolver>::TypeId,
        _type_resolver: &Resolver,
        _out: &mut Vec<u8>,
    ) -> Result<(), TransactionExtensionsError> {
        Err(TransactionExtensionsError::NotFound(name.to_owned()))
    }
}

// Non-empty tuples impl `TransactionExtensions`: for each extension we do a linear
// search through the tuple items to find it and call the appropriate encode method.
macro_rules! impl_tuples {
    ($($ident:ident $index:tt),*) => {
        impl <Resolver: TypeResolver $(,$ident)*> TransactionExtensions<Resolver> for ($($ident,)*)
        where
            $($ident: TransactionExtension<Resolver>,)*
        {
            fn contains_extension(&self, name: &str) -> bool {
                $(
                    if $ident::NAME == name {
                        return true
                    }
                )*
                false
            }

            fn is_authorization_extension(&self, name: &str) -> bool {
                $(
                    if $ident::NAME == name {
                        return self.$index.is_authorization_extension()
                    }
                )*
                false
            }

            fn encode_extension_value_to(
                &self,
                name: &str,
                type_id: <Resolver as TypeResolver>::TypeId,
                type_resolver: &Resolver,
                out: &mut Vec<u8>
            ) -> Result<(), TransactionExtensionsError> {
                let len = out.len();

                $(
                    if $ident::NAME == name {
                        return self.$index.encode_value_to(type_id, type_resolver, out)
                            .map_err(|e| {
                                // Protection: if we are returning an error then
                                // no bytes should have been encoded to the given
                                // Vec. Ensure that this is true:
                                out.truncate(len);
                                TransactionExtensionsError::Other {
                                    extension_name: name.to_owned(),
                                    error: e,
                                }
                            });
                    }
                )*

                Err(TransactionExtensionsError::NotFound(name.to_owned()))
            }

            fn encode_extension_implicit_to(
                &self,
                name: &str,
                type_id: <Resolver as TypeResolver>::TypeId,
                type_resolver: &Resolver,
                out: &mut Vec<u8>
            ) -> Result<(), TransactionExtensionsError> {
                let len = out.len();

                $(
                    if $ident::NAME == name {
                        return self.$index.encode_implicit_to(type_id, type_resolver, out)
                            .map_err(|e| {
                                // Protection: if we are returning an error then
                                // no bytes should have been encoded to the given
                                // Vec. Ensure that this is true:
                                out.truncate(len);
                                TransactionExtensionsError::Other {
                                    extension_name: name.to_owned(),
                                    error: e,
                                }
                            });
                    }
                )*

                Err(TransactionExtensionsError::NotFound(name.to_owned()))
            }
        }
    }
}

#[rustfmt::skip]
const _: () = {
    impl_tuples!(A 0);
    impl_tuples!(A 0, B 1);
    impl_tuples!(A 0, B 1, C 2);
    impl_tuples!(A 0, B 1, C 2, D 3);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20, V 21);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20, V 21, W 22);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20, V 21, W 22, X 23);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20, V 21, W 22, X 23, Y 24);
    impl_tuples!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20, V 21, W 22, X 23, Y 24, Z 25);
};
