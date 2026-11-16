// Copyright (C) 2024 Parity Technologies (UK) Ltd. (admin@parity.io)
// This file is a part of the scale-info-legacy crate.
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

//! This crate provides a set of types which build on each other. The ultimate goal is to be able to define
//! the necessary type information needed to describe how historic Substrate types are SCALE encoded.
//!
//! The main types exposed here are as follows:
//!
//! - [`TypeRegistry`]: the lowest level type which one can populate with type information (via [`TypeRegistry::insert()`]
//!   and [`TypeRegistry::insert_runtime_api()`]), and then query to resolve some type name to the relevant information
//!   (via [`TypeRegistry::resolve_type()`] and [`TypeRegistry::runtime_api()`]).
//! - [`TypeRegistrySet`]: a set of the above, which will resolve types (via [`TypeRegistrySet::resolve_type()`]) by working
//!   through the inner type registries until it finds the relevant information (or doesn't find anything). This allows us
//!   to combine type registries in different ways to alter how we resolve things.
//! - [`ChainTypeRegistry`]: this type is constructed by deserializing some JSON (or similar) which describes all of the
//!   types that we need to know about on a given chain. The main function here is [`ChainTypeRegistry::for_spec_version()`],
//!   which returns a [`TypeRegistrySet`] for resolving types for the given spec version.
//!
//! We also expose an [`InsertName`], which is built by parsing type names like `Vec<T>` from strings, and used in
//! [`TypeRegistry::insert()`] to insert types, and then [`LookupName`], which is built by parsing type names like
//! `Vec<T>`, `[u8; 32]` and `(bool, u32)` and is used to lookup the corresponding type information via [`TypeRegistry::resolve_type()`]
//! and similar. Finally, [`TypeShape`] is an enum used to describe the shape of the type inserted via [`TypeRegistry::insert()`].
//!
//! This crate, like `scale-info`, can be used in conjunction with crates like:
//!
//! - [`scale-decode`](https://github.com/paritytech/scale-decode) to decode SCALE encoded bytes
//!   into custom types based on this type information.
//! - [`scale-encode`](https://github.com/paritytech/scale-encode) to SCALE encode custom types
//!   into bytes based on this type information.
//! - [`scale-value`](https://github.com/paritytech/scale-value) to SCALE encode or decode from a
//!   `Value` type (a bit like `serde_json`'s `Value` type).

#![no_std]
#![deny(missing_docs)]

extern crate alloc;

pub mod chain_types;
pub mod insert_name;
pub mod lookup_name;
pub mod type_registry;
pub mod type_registry_set;
pub mod type_shape;

#[cfg(test)]
mod test_utils;

// Export the main types here for ease of use:
pub use {
    chain_types::ChainTypeRegistry, insert_name::InsertName, lookup_name::LookupName,
    type_registry::RuntimeApiInput, type_registry::TypeRegistry,
    type_registry_set::TypeRegistrySet, type_shape::TypeShape,
};
