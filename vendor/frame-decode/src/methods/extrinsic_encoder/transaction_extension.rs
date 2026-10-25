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

use alloc::boxed::Box;
use alloc::vec::Vec;
use scale_type_resolver::TypeResolver;

/// This can be implemented for anything which is a valid Substrate transaction extension.
/// Transaction extensions each have a unique name to identify them, and are able to encode
/// explicit `value` bytes to a transaction, or "implicit" bytes to a transaction signer payload.
pub trait TransactionExtension<Resolver: TypeResolver> {
    /// The name of this transaction extension.
    const NAME: &str;

    /// Does this extension authorize the transaction, eg by containing a signature
    /// like `VerifyMultiSignature` does? Such an extension signs the transaction
    /// extension version, the call data, and the values and implicits of only the
    /// extensions _after_ it, and so when a V5 signer payload is built, the value
    /// and implicit bytes of an authorization extension, and of every extension
    /// before it, are excluded from the payload.
    ///
    /// This defaults to `false`, which is correct for any extension which doesn't
    /// authorize the transaction.
    fn is_authorization_extension(&self) -> bool {
        false
    }

    /// Given type information for the expected transaction extension,
    /// this should encode the value (ie the bytes that will appear in the
    /// transaction) to the provided `Vec`, or encode nothing and emit an error.
    fn encode_value_to(
        &self,
        type_id: Resolver::TypeId,
        type_resolver: &Resolver,
        out: &mut Vec<u8>,
    ) -> Result<(), TransactionExtensionError>;

    /// Given type information for the expected transaction extension,
    /// this should encode the implicit (ie the bytes that will appear in the
    /// signer payload) to the provided `Vec`, or encode nothing and emit an error.
    fn encode_implicit_to(
        &self,
        type_id: Resolver::TypeId,
        type_resolver: &Resolver,
        out: &mut Vec<u8>,
    ) -> Result<(), TransactionExtensionError>;
}

/// This error will be returned if any of the methods in [`TransactionExtension`] fail.
pub type TransactionExtensionError = Box<dyn core::error::Error + Send + Sync + 'static>;
