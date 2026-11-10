// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Parse the complete module while keeping custom metadata opaque.

use core::convert::Infallible;
use wasm_encoder::{
	reencode::{self, Reencode},
	Module,
};

pub(crate) fn has_runtime_version_section(
	bytes: &[u8],
) -> Result<bool, reencode::Error<Infallible>> {
	#[derive(Default)]
	struct Metadata {
		has_runtime_version: bool,
	}
	impl Reencode for Metadata {
		type Error = Infallible;
		fn parse_custom_section(
			&mut self,
			_module: &mut Module,
			section: wasmparser::CustomSectionReader<'_>,
		) -> Result<(), reencode::Error<Self::Error>> {
			self.has_runtime_version |= section.name() == "runtime_version";
			// The old parser did not interpret optional name/debug metadata. The
			// encoded output is discarded, so no custom payload needs copying.
			Ok(())
		}
	}
	let mut metadata = Metadata::default();
	// Parser::parse_all alone only outlines sections and function bodies. Full
	// reencoding also parses their contents, preserving the former malformed-
	// module rejection without adding stricter semantic validation.
	metadata.parse_core_module(&mut Module::new(), wasmparser::Parser::new(0), bytes)?;
	Ok(metadata.has_runtime_version)
}

#[cfg(test)]
mod tests {
	use super::*;
	use wasm_encoder::{CustomSection, ExportKind, ExportSection, RawSection};

	fn with_runtime_version() -> Module {
		let mut module = Module::new();
		module.section(&CustomSection {
			name: "runtime_version".into(),
			data: (&b"version"[..]).into(),
		});
		module
	}

	#[test]
	fn missing_runtime_version_is_reported() {
		assert!(!has_runtime_version_section(&Module::new().finish()).unwrap());
	}

	#[test]
	fn malformed_instruction_is_rejected_even_after_metadata_was_found() {
		let mut module = with_runtime_version();
		module.section(&RawSection { id: 1, data: &[1, 0x60, 0, 0] });
		module.section(&RawSection { id: 3, data: &[1, 0] });
		module.section(&RawSection { id: 10, data: &[1, 3, 0, 0xff, 0x0b] });
		assert!(has_runtime_version_section(&module.finish()).is_err());
	}

	#[test]
	fn malformed_section_entry_is_rejected_even_after_metadata_was_found() {
		let mut module = with_runtime_version();
		module.section(&RawSection { id: 1, data: &[1, 0xff] });
		assert!(has_runtime_version_section(&module.finish()).is_err());
	}

	#[test]
	fn optional_name_metadata_remains_opaque() {
		let mut module = with_runtime_version();
		module.section(&CustomSection { name: "name".into(), data: (&b"\xff\xff"[..]).into() });
		assert!(has_runtime_version_section(&module.finish()).unwrap());
	}

	#[test]
	fn parsing_does_not_add_semantic_type_validation() {
		let mut module = with_runtime_version();
		module.section(ExportSection::new().export("unresolved", ExportKind::Func, 99));
		assert!(has_runtime_version_section(&module.finish()).unwrap());
	}
}
