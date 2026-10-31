// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

//! Memory rewrites using the maintained Bytecode Alliance parser and encoder.
use crate::{error::WasmError, wasm_runtime::HeapAllocStrategy};
use wasm_encoder::{
	reencode::{self, Reencode},
	ExportKind, ExportSection, ImportSection, MemorySection, Module, SectionId,
};
use wasmparser::{Parser, TypeRef};

type Error = reencode::Error<String>;

#[derive(Default)]
pub(super) struct Transform {
	convert: bool,
	converted: Option<(wasmparser::MemoryType, String)>,
	memory_emitted: bool,
	export_emitted: bool,
	heap: Option<HeapAllocStrategy>,
	clear_maximum: bool,
}

impl Transform {
	pub(super) fn convert_memory() -> Self {
		Self { convert: true, ..Self::default() }
	}
	pub(super) fn heap(strategy: HeapAllocStrategy) -> Self {
		Self { heap: Some(strategy), ..Self::default() }
	}
	pub(super) fn clear_maximum() -> Self {
		Self { clear_maximum: true, ..Self::default() }
	}
	pub(super) fn run(mut self, bytes: &[u8]) -> Result<Vec<u8>, WasmError> {
		let mut module = Module::new();
		self.parse_core_module(&mut module, Parser::new(0), bytes)
			.map_err(|error| WasmError::Other(format!("cannot rewrite module memory: {error}")))?;
		if self.heap.is_some() && !self.memory_emitted {
			return Err(WasmError::Other("no memory section found".into()));
		}
		Ok(module.finish())
	}
	fn append_export(&mut self, exports: &mut ExportSection) {
		if let Some((_, name)) = &self.converted {
			exports.export(name, ExportKind::Memory, 0);
		}
		self.export_emitted = true;
	}
}

impl Reencode for Transform {
	type Error = String;
	fn parse_custom_section(
		&mut self,
		module: &mut Module,
		section: wasmparser::CustomSectionReader<'_>,
	) -> Result<(), Error> {
		module.section(&wasm_encoder::CustomSection {
			name: section.name().into(),
			data: section.data().into(),
		});
		Ok(())
	}
	fn parse_import_section(
		&mut self,
		imports: &mut ImportSection,
		section: wasmparser::ImportSectionReader<'_>,
	) -> Result<(), Error> {
		for import in section.into_imports() {
			let import = import?;
			if self.convert && self.converted.is_none() {
				if let TypeRef::Memory(memory) = import.ty {
					self.converted = Some((memory, import.name.to_owned()));
					continue;
				}
			}
			imports.import(import.module, import.name, self.entity_type(import.ty)?);
		}
		Ok(())
	}
	fn parse_memory_section(
		&mut self,
		memories: &mut MemorySection,
		section: wasmparser::MemorySectionReader<'_>,
	) -> Result<(), Error> {
		if self.converted.is_some() {
			return Err(Error::UserError(
				"cannot convert a memory import when a memory section already exists".into(),
			));
		}
		if self.heap.is_some() && section.count() == 0 {
			return Err(Error::UserError("memory section is empty".into()));
		}
		for memory in section {
			let mut memory = memory?;
			if self.clear_maximum {
				memory.maximum = None;
			}
			if let Some(strategy) = self.heap {
				// The runtime heap API and its historical saturating addition use wasm32 pages.
				if memory.memory64 || memory.shared || memory.page_size_log2.is_some() {
					return Err(Error::UserError(
						"runtime heap requires an unshared wasm32 memory with standard pages"
							.into(),
					));
				}
				let initial = u32::try_from(memory.initial)
					.map_err(|_| Error::UserError("memory minimum exceeds wasm32 pages".into()))?;
				let (minimum, maximum) = match strategy {
					HeapAllocStrategy::Dynamic { maximum_pages } => (
						maximum_pages.map(|max| max.min(initial)).unwrap_or(initial),
						maximum_pages,
					),
					HeapAllocStrategy::Static { extra_pages } => {
						let pages = initial.saturating_add(extra_pages);
						(pages, Some(pages))
					},
				};
				memory.initial = u64::from(minimum);
				memory.maximum = maximum.map(u64::from);
			}
			memories.memory(self.memory_type(memory)?);
		}
		self.memory_emitted = true;
		Ok(())
	}
	fn parse_export_section(
		&mut self,
		exports: &mut ExportSection,
		section: wasmparser::ExportSectionReader<'_>,
	) -> Result<(), Error> {
		reencode::utils::parse_export_section(self, exports, section)?;
		self.append_export(exports);
		Ok(())
	}
	fn intersperse_section_hook(
		&mut self,
		module: &mut Module,
		_after: Option<SectionId>,
		before: Option<SectionId>,
	) -> Result<(), Error> {
		if let Some((memory, _)) = &self.converted {
			if !self.memory_emitted &&
				before.map(|id| id as u8 > SectionId::Memory as u8).unwrap_or(true)
			{
				let mut memories = MemorySection::new();
				memories.memory(self.memory_type(*memory)?);
				module.section(&memories);
				self.memory_emitted = true;
			}
			if !self.export_emitted &&
				before
					.map(|id| id != SectionId::Tag && id as u8 > SectionId::Export as u8)
					.unwrap_or(true)
			{
				let mut exports = ExportSection::new();
				self.append_export(&mut exports);
				module.section(&exports);
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::runtime_blob::RuntimeBlob;
	use wasm_encoder::{EntityType, MemoryType};
	use wasmparser::Payload;

	fn memory(minimum: u64, maximum: Option<u64>) -> MemoryType {
		MemoryType { minimum, maximum, memory64: false, shared: false, page_size_log2: None }
	}
	fn module_with_memory(minimum: u64, maximum: Option<u64>) -> RuntimeBlob {
		let mut module = Module::new();
		module.section(MemorySection::new().memory(memory(minimum, maximum)));
		RuntimeBlob::new(&module.finish()).unwrap()
	}
	fn memories(blob: RuntimeBlob) -> Vec<(u64, Option<u64>)> {
		Parser::new(0)
			.parse_all(&blob.serialize())
			.filter_map(|p| match p.unwrap() {
				Payload::MemorySection(section) => Some(
					section
						.into_iter()
						.map(|m| {
							let m = m.unwrap();
							(m.initial, m.maximum)
						})
						.collect::<Vec<_>>(),
				),
				_ => None,
			})
			.flatten()
			.collect()
	}
	#[test]
	#[allow(deprecated, reason = "regression coverage for the retained legacy heap rewrite API")]
	fn heap_preserves_historical_dynamic_clamp_and_static_saturation() {
		for (strategy, expected) in [
			(HeapAllocStrategy::Dynamic { maximum_pages: None }, (10, None)),
			(HeapAllocStrategy::Dynamic { maximum_pages: Some(5) }, (5, Some(5))),
			(HeapAllocStrategy::Dynamic { maximum_pages: Some(20) }, (10, Some(20))),
			(HeapAllocStrategy::Static { extra_pages: 7 }, (17, Some(17))),
			(
				HeapAllocStrategy::Static { extra_pages: u32::MAX },
				(u32::MAX as u64, Some(u32::MAX as u64)),
			),
		] {
			let mut blob = module_with_memory(10, Some(30));
			blob.setup_memory_according_to_heap_alloc_strategy(strategy).unwrap();
			assert_eq!(memories(blob), vec![expected]);
		}
	}
	#[test]
	fn clear_maximum_preserves_initial_pages_and_accepts_absent_memory() {
		let mut module = Module::new();
		module.section(MemorySection::new().memory(memory(3, Some(8))).memory(memory(7, Some(12))));
		module.section(&wasm_encoder::CustomSection {
			name: "opaque".into(),
			data: (&b"\xff\xff"[..]).into(),
		});
		let mut blob = RuntimeBlob::new(&module.finish()).unwrap();
		blob.clear_memory_max_limit().unwrap();
		assert_eq!(blob.custom_section_contents("opaque"), Some(&b"\xff\xff"[..]));
		assert_eq!(memories(blob.clone()), vec![(3, None), (7, None)]);
		let once = blob.clone().serialize();
		blob.clear_memory_max_limit().unwrap();
		assert_eq!(blob.serialize(), once);

		let empty = Module::new().finish();
		let mut blob = RuntimeBlob::new(&empty).unwrap();
		blob.clear_memory_max_limit().unwrap();
		assert_eq!(blob.serialize(), empty);
	}
	#[test]
	fn import_conversion_creates_sections_preserves_other_imports_and_custom_data() {
		let mut module = Module::new();
		module.section(ImportSection::new().import("env", "host", EntityType::Function(0)).import(
			"env",
			"linear",
			EntityType::Memory(memory(3, Some(8))),
		));
		module.section(&wasm_encoder::CustomSection {
			name: "name".into(),
			data: (&b"\xff\xff"[..]).into(),
		});
		let mut blob = RuntimeBlob::new(&module.finish()).unwrap();
		blob.convert_memory_import_into_export().unwrap();
		assert_eq!(blob.custom_section_contents("name"), Some(&b"\xff\xff"[..]));
		assert_eq!(memories(blob.clone()), vec![(3, Some(8))]);
		let mut imports = vec![];
		let mut exports = vec![];
		for payload in Parser::new(0).parse_all(&blob.serialize()) {
			match payload.unwrap() {
				Payload::ImportSection(s) => {
					imports.extend(s.into_imports().map(|i| i.unwrap().name.to_owned()))
				},
				Payload::ExportSection(s) => exports.extend(s.into_iter().map(|e| {
					let e = e.unwrap();
					(e.name.to_owned(), e.kind, e.index)
				})),
				_ => {},
			}
		}
		assert_eq!(imports, vec!["host"]);
		assert_eq!(exports, vec![("linear".into(), wasmparser::ExternalKind::Memory, 0)]);
	}
	#[test]
	fn conversion_appends_to_existing_exports_and_is_idempotent() {
		let mut module = Module::new();
		module.section(ImportSection::new().import(
			"env",
			"memory",
			EntityType::Memory(memory(1, None)),
		));
		module.section(ExportSection::new().export("existing", ExportKind::Func, 0));
		let mut blob = RuntimeBlob::new(&module.finish()).unwrap();
		blob.convert_memory_import_into_export().unwrap();
		let once = blob.clone().serialize();
		blob.convert_memory_import_into_export().unwrap();
		assert_eq!(once, blob.serialize());
		let names = Parser::new(0)
			.parse_all(&once)
			.filter_map(|p| match p.unwrap() {
				Payload::ExportSection(s) => {
					Some(s.into_iter().map(|e| e.unwrap().name.to_owned()).collect::<Vec<_>>())
				},
				_ => None,
			})
			.flatten()
			.collect::<Vec<_>>();
		assert_eq!(names, vec!["existing", "memory"]);
	}
	#[test]
	fn converted_export_follows_the_logical_tag_section_order() {
		let mut module = Module::new();
		module.section(ImportSection::new().import(
			"env",
			"memory",
			EntityType::Memory(memory(1, None)),
		));
		module.section(wasm_encoder::TagSection::new().tag(wasm_encoder::TagType {
			kind: wasm_encoder::TagKind::Exception,
			func_type_idx: 0,
		}));
		let mut blob = RuntimeBlob::new(&module.finish()).unwrap();
		blob.convert_memory_import_into_export().unwrap();
		let bytes = blob.serialize();
		let ids: Vec<_> = Parser::new(0)
			.parse_all(&bytes)
			.filter_map(|p| p.unwrap().as_section().map(|(id, _)| id))
			.collect();
		assert_eq!(ids, vec![2, 5, 13, 7]);
	}
	#[test]
	#[allow(deprecated, reason = "regression coverage for the retained legacy heap rewrite API")]
	fn invalid_memory_rewrites_fail_without_mutating_the_blob() {
		let mut empty = RuntimeBlob::new(&Module::new().finish()).unwrap();
		assert!(empty
			.setup_memory_according_to_heap_alloc_strategy(HeapAllocStrategy::Static {
				extra_pages: 1
			})
			.is_err());
		let mut module = Module::new();
		module.section(&MemorySection::new());
		let mut empty_section = RuntimeBlob::new(&module.finish()).unwrap();
		assert!(empty_section
			.setup_memory_according_to_heap_alloc_strategy(HeapAllocStrategy::Dynamic {
				maximum_pages: None
			})
			.is_err());
		let mut module = Module::new();
		module.section(ImportSection::new().import(
			"env",
			"memory",
			EntityType::Memory(memory(1, None)),
		));
		module.section(MemorySection::new().memory(memory(1, None)));
		let mut conflict = RuntimeBlob::new(&module.finish()).unwrap();
		let original = conflict.clone().serialize();
		assert!(conflict.convert_memory_import_into_export().is_err());
		assert_eq!(conflict.serialize(), original);
	}
}
