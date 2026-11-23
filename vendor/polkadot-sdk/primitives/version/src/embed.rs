// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Provides functionality to embed a [`RuntimeVersion`](crate::RuntimeVersion) as custom section
//! into a WASM file.

use codec::Encode;
use std::borrow::Cow;
use wasm_encoder::{
	reencode::{self, Reencode},
	CustomSection, Module,
};

#[derive(Clone, Copy, Eq, PartialEq, Debug, thiserror::Error)]
pub enum Error {
	#[error("Deserializing wasm failed")]
	Deserialize,
	#[error("Serializing wasm failed")]
	Serialize,
}

/// Embed the given `version` to the given `wasm` blob.
///
/// If there was already a runtime version embedded, this will be overwritten.
///
/// Returns the new WASM blob.
pub fn embed_runtime_version(
	wasm: &[u8],
	mut version: crate::RuntimeVersion,
) -> Result<Vec<u8>, Error> {
	let mut module = Module::new();

	let apis = version
		.apis
		.iter()
		.map(Encode::encode)
		.flat_map(|v| v.into_iter())
		.collect::<Vec<u8>>();

	version.apis.to_mut().clear();
	let mut embed = Embed { apis: Some(apis), version: Some(version.encode()) };
	embed
		.parse_core_module(&mut module, wasmparser::Parser::new(0), wasm)
		.map_err(|_| Error::Deserialize)?;
	for (name, data) in [("runtime_apis", embed.apis), ("runtime_version", embed.version)] {
		if let Some(data) = data {
			module.section(&CustomSection { name: name.into(), data: data.into() });
		}
	}
	Ok(module.finish())
}

// Keep custom sections opaque, including malformed optional debug/name metadata. Replace
// only the first matching section, as the previous serializer did.
struct Embed {
	apis: Option<Vec<u8>>,
	version: Option<Vec<u8>>,
}
impl Reencode for Embed {
	type Error = core::convert::Infallible;
	fn parse_custom_section(
		&mut self,
		module: &mut Module,
		section: wasmparser::CustomSectionReader<'_>,
	) -> Result<(), reencode::Error<Self::Error>> {
		let replacement = match section.name() {
			"runtime_apis" => self.apis.take(),
			"runtime_version" => self.version.take(),
			_ => None,
		};
		module.section(&CustomSection {
			name: section.name().into(),
			data: replacement.map(Cow::Owned).unwrap_or_else(|| Cow::Borrowed(section.data())),
		});
		Ok(())
	}
}

#[cfg(test)]
mod migration_tests {
	use super::*;
	fn custom(module: &mut Module, name: &'static str, data: &'static [u8]) {
		module.section(&CustomSection { name: name.into(), data: data.into() });
	}
	fn sections(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
		wasmparser::Parser::new(0)
			.parse_all(bytes)
			.filter_map(|payload| match payload.unwrap() {
				wasmparser::Payload::CustomSection(section) => {
					Some((section.name().into(), section.data().into()))
				},
				_ => None,
			})
			.collect()
	}
	#[test]
	fn replaces_first_metadata_preserves_duplicates_and_opaque_custom_sections() {
		let mut module = Module::new();
		custom(&mut module, "name", b"\xff\xff");
		custom(&mut module, "runtime_version", b"old");
		custom(&mut module, "runtime_version", b"duplicate");
		custom(&mut module, "runtime_apis", b"old apis");
		custom(&mut module, "other", b"untouched");
		let mut version = crate::RuntimeVersion {
			spec_version: 4812,
			apis: vec![(*b"12345678", 7)].into(),
			..Default::default()
		};
		let output = embed_runtime_version(&module.finish(), version.clone()).unwrap();
		let api_bytes = version.apis[0].encode();
		version.apis.to_mut().clear();
		assert_eq!(
			sections(&output),
			vec![
				("name".into(), b"\xff\xff".to_vec()),
				("runtime_version".into(), version.encode()),
				("runtime_version".into(), b"duplicate".to_vec()),
				("runtime_apis".into(), api_bytes),
				("other".into(), b"untouched".to_vec()),
			]
		);
	}
	#[test]
	fn appends_missing_metadata_in_historical_order() {
		let mut version = crate::RuntimeVersion::default();
		version.apis = vec![(*b"abcdefgh", 2)].into();
		let api = version.apis[0].encode();
		let bytes = embed_runtime_version(&Module::new().finish(), version.clone()).unwrap();
		version.apis.to_mut().clear();
		assert_eq!(
			sections(&bytes),
			vec![("runtime_apis".into(), api), ("runtime_version".into(), version.encode())]
		);
	}
	#[test]
	fn rejects_invalid_wasm_and_invalid_function_instructions() {
		assert_eq!(embed_runtime_version(b"invalid", Default::default()), Err(Error::Deserialize));
		let mut module = Module::new();
		module.section(&wasm_encoder::RawSection { id: 10, data: &[1, 3, 0, 0xff, 0x0b] });
		assert_eq!(
			embed_runtime_version(&module.finish(), Default::default()),
			Err(Error::Deserialize)
		);
	}
}
