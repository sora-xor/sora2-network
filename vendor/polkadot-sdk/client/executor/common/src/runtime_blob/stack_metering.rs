// SORA Nexus maintenance migration: stack-limiter policy from wasm-instrument 0.4.0
// (Parity Technologies, MIT OR Apache-2.0), implemented using maintained
// wasmparser/wasm-encoder. See SORA-PATCH.md for compatibility evidence.

use std::collections::{BTreeMap, BTreeSet};
use wasm_encoder::{
	reencode::{self, Reencode},
	Instruction as I,
};
use wasmparser::{BlockType, FuncType, Operator, Parser, Payload, TypeRef};

type Result<T> = std::result::Result<T, String>;
type EncodeResult<T> = std::result::Result<T, reencode::Error<String>>;

#[derive(Default)]
struct Meter {
	types: Vec<FuncType>,
	function_types: Vec<u32>,
	costs: Vec<u32>,
	thunks: BTreeMap<u32, u32>,
	global: u32,
	global_added: bool,
	limit: u32,
	custom_sections: Vec<(String, Vec<u8>)>,
}

pub(super) fn inject(raw: &[u8], limit: u32) -> Result<Vec<u8>> {
	let mut meter = Meter { limit, ..Meter::default() };
	let mut bodies = Vec::new();
	let mut entrypoints = BTreeSet::new();
	for payload in Parser::new(0).parse_all(raw) {
		match payload.map_err(|e| e.to_string())? {
			Payload::Version { encoding: wasmparser::Encoding::Module, .. } => {},
			Payload::Version { .. } => return Err("expected a core Wasm module".into()),
			Payload::TypeSection(section) => {
				for ty in section.into_iter_err_on_gc_types() {
					let ty = ty.map_err(|e| e.to_string())?;
					if !ty.params().iter().chain(ty.results()).all(numeric_type) {
						return Err("unsupported value type in stack metering".into());
					}
					meter.types.push(ty);
				}
			},
			Payload::ImportSection(section) => {
				for import in section.into_imports() {
					match import.map_err(|e| e.to_string())?.ty {
						TypeRef::Func(index) => {
							meter.function_types.push(index);
							meter.costs.push(0);
						},
						// The historical limiter used a defined-global index here, ignoring
						// imports. Reject that unsupported case instead of misdirecting writes.
						TypeRef::Global(_) => {
							return Err("stack metering does not support imported globals".into())
						},
						TypeRef::Memory(_) | TypeRef::Table(_) => {},
						_ => return Err("unsupported import in stack metering".into()),
					}
				}
			},
			Payload::FunctionSection(section) => {
				for index in section {
					meter.function_types.push(index.map_err(|e| e.to_string())?);
				}
			},
			Payload::GlobalSection(section) => meter.global = section.count(),
			Payload::ExportSection(section) => {
				for export in section {
					let export = export.map_err(|e| e.to_string())?;
					if export.kind == wasmparser::ExternalKind::Func {
						entrypoints.insert(export.index);
					}
				}
			},
			Payload::StartSection { func, .. } => {
				entrypoints.insert(func);
			},
			Payload::ElementSection(section) => {
				for element in section {
					match element.map_err(|e| e.to_string())?.items {
						wasmparser::ElementItems::Functions(functions) => {
							for index in functions {
								entrypoints.insert(index.map_err(|e| e.to_string())?);
							}
						},
						_ => {
							return Err("unsupported reference expression in stack metering".into())
						},
					}
				}
			},
			Payload::CodeSectionEntry(body) => bodies.push(body),
			Payload::TagSection(_) => {
				return Err("exception tags are unsupported in stack metering".into())
			},
			_ => {},
		}
	}
	if meter.costs.len().checked_add(bodies.len()) != Some(meter.function_types.len()) {
		return Err("function and code section lengths differ".into());
	}
	for body in bodies {
		let ty = meter.function_type(meter.costs.len() as u32)?;
		let mut locals: u32 = 0;
		for local in body.get_locals_reader().map_err(|e| e.to_string())? {
			let (count, ty) = local.map_err(|e| e.to_string())?;
			if !numeric_type(&ty) {
				return Err("unsupported local type in stack metering".into());
			}
			locals = locals.checked_add(count).ok_or("overflow in local count")?;
		}
		let height = maximum_height(&body, ty.results().len() as u32, &meter)?;
		meter.costs.push(locals.checked_add(height).ok_or("stack cost overflow")?);
	}
	let mut next = u32::try_from(meter.function_types.len()).map_err(|_| "too many functions")?;
	for function in entrypoints {
		if *meter
			.costs
			.get(function as usize)
			.ok_or("entrypoint function is out of bounds")? !=
			0
		{
			meter.thunks.insert(function, next);
			next = next.checked_add(1).ok_or("too many thunks")?;
		}
	}
	let mut output = wasm_encoder::Module::new();
	meter
		.parse_core_module(&mut output, Parser::new(0), raw)
		.map_err(|e| e.to_string())?;
	// Preserve the former thunk builder's ordering, including duplicate custom
	// section names: it moved custom sections to the end in reverse input order.
	for (name, data) in meter.custom_sections.into_iter().rev() {
		output.section(&wasm_encoder::CustomSection { name: name.into(), data: data.into() });
	}
	Ok(output.finish())
}

impl Meter {
	fn function_type(&self, function: u32) -> Result<&FuncType> {
		let index =
			*self.function_types.get(function as usize).ok_or("function is out of bounds")?;
		self.types
			.get(index as usize)
			.ok_or_else(|| "function type is out of bounds".into())
	}

	fn replacement(&self, function: u32) -> u32 {
		self.thunks.get(&function).copied().unwrap_or(function)
	}

	fn add_global(&mut self, globals: &mut wasm_encoder::GlobalSection) {
		globals.global(
			wasm_encoder::GlobalType {
				val_type: wasm_encoder::ValType::I32,
				mutable: true,
				shared: false,
			},
			&wasm_encoder::ConstExpr::i32_const(0),
		);
		self.global_added = true;
	}

	fn call(&self, function: &mut wasm_encoder::Function, callee: u32, cost: u32) {
		for instruction in [
			I::GlobalGet(self.global),
			I::I32Const(cost as i32),
			I::I32Add,
			I::GlobalSet(self.global),
			I::GlobalGet(self.global),
			I::I32Const(self.limit as i32),
			I::I32GtU,
			I::If(wasm_encoder::BlockType::Empty),
			I::Unreachable,
			I::End,
			I::Call(callee),
			I::GlobalGet(self.global),
			I::I32Const(cost as i32),
			I::I32Sub,
			I::GlobalSet(self.global),
		] {
			function.instruction(&instruction);
		}
	}
}

impl Reencode for Meter {
	type Error = String;

	fn parse_custom_section(
		&mut self,
		_module: &mut wasm_encoder::Module,
		section: wasmparser::CustomSectionReader<'_>,
	) -> EncodeResult<()> {
		// Debug/name metadata was opaque to the old instrumenter. Rewriting it can
		// reject historical payloads that were never validated as executable code.
		self.custom_sections.push((section.name().into(), section.data().into()));
		Ok(())
	}

	fn parse_function_section(
		&mut self,
		functions: &mut wasm_encoder::FunctionSection,
		section: wasmparser::FunctionSectionReader<'_>,
	) -> EncodeResult<()> {
		reencode::utils::parse_function_section(self, functions, section)?;
		for callee in self.thunks.keys() {
			let ty = self.function_type(*callee).map_err(reencode::Error::UserError)?;
			// The former builder selected the first structurally identical signature.
			let index = self
				.types
				.iter()
				.position(|candidate| candidate == ty)
				.ok_or_else(|| reencode::Error::UserError("thunk type is missing".into()))?;
			functions.function(index as u32);
		}
		Ok(())
	}

	fn parse_global_section(
		&mut self,
		globals: &mut wasm_encoder::GlobalSection,
		section: wasmparser::GlobalSectionReader<'_>,
	) -> EncodeResult<()> {
		reencode::utils::parse_global_section(self, globals, section)?;
		self.add_global(globals);
		Ok(())
	}

	fn intersperse_section_hook(
		&mut self,
		module: &mut wasm_encoder::Module,
		_after: Option<wasm_encoder::SectionId>,
		before: Option<wasm_encoder::SectionId>,
	) -> EncodeResult<()> {
		if !self.global_added &&
			before.map_or(true, |section| section as u8 > wasm_encoder::SectionId::Global as u8)
		{
			let mut globals = wasm_encoder::GlobalSection::new();
			self.add_global(&mut globals);
			module.section(&globals);
		}
		Ok(())
	}

	fn parse_export(
		&mut self,
		exports: &mut wasm_encoder::ExportSection,
		export: wasmparser::Export<'_>,
	) -> EncodeResult<()> {
		let index = if export.kind == wasmparser::ExternalKind::Func {
			self.replacement(export.index)
		} else {
			export.index
		};
		exports.export(export.name, self.export_kind(export.kind)?, index);
		Ok(())
	}

	fn element_items<'a>(
		&mut self,
		items: wasmparser::ElementItems<'a>,
	) -> EncodeResult<wasm_encoder::Elements<'a>> {
		match items {
			wasmparser::ElementItems::Functions(functions) => {
				let mut mapped = Vec::new();
				for function in functions {
					mapped.push(self.replacement(function?));
				}
				Ok(wasm_encoder::Elements::Functions(mapped.into()))
			},
			_ => Err(reencode::Error::UserError("unsupported element expression".into())),
		}
	}

	fn start_section(&mut self, start: u32) -> EncodeResult<u32> {
		Ok(self.replacement(start))
	}

	fn parse_function_body(
		&mut self,
		code: &mut wasm_encoder::CodeSection,
		body: wasmparser::FunctionBody<'_>,
	) -> EncodeResult<()> {
		let mut function = self.new_function_with_parsed_locals(&body)?;
		for operator in body.get_operators_reader()? {
			let operator = operator?;
			if let Operator::Call { function_index } = operator {
				let cost = *self
					.costs
					.get(function_index as usize)
					.ok_or_else(|| reencode::Error::UserError("callee is out of bounds".into()))?;
				if cost != 0 {
					self.call(&mut function, function_index, cost);
					continue;
				}
			}
			function.instruction(&self.instruction(operator)?);
		}
		code.function(&function);
		Ok(())
	}

	fn parse_code_section(
		&mut self,
		code: &mut wasm_encoder::CodeSection,
		section: wasmparser::CodeSectionReader<'_>,
	) -> EncodeResult<()> {
		reencode::utils::parse_code_section(self, code, section)?;
		for callee in self.thunks.keys() {
			let ty = self.function_type(*callee).map_err(reencode::Error::UserError)?;
			let mut function = wasm_encoder::Function::new([]);
			for argument in 0..ty.params().len() {
				function.instruction(&I::LocalGet(argument as u32));
			}
			self.call(&mut function, *callee, self.costs[*callee as usize]);
			function.instruction(&I::End);
			code.function(&function);
		}
		Ok(())
	}
}

struct Frame {
	polymorphic: bool,
	end: u32,
	branch: u32,
	start: u32,
}
struct Stack {
	height: u32,
	frames: Vec<Frame>,
}

impl Stack {
	fn frame(&self, depth: u32) -> Result<&Frame> {
		let distance = (depth as usize).checked_add(1).ok_or("control stack depth overflow")?;
		let index = self.frames.len().checked_sub(distance).ok_or("control stack out of bounds")?;
		Ok(&self.frames[index])
	}
	fn unreachable(&mut self) -> Result<()> {
		self.frames.last_mut().ok_or("empty control stack")?.polymorphic = true;
		Ok(())
	}
	fn push(&mut self, count: u32) -> Result<()> {
		self.height = self.height.checked_add(count).ok_or("operand stack overflow")?;
		Ok(())
	}
	fn pop(&mut self, count: u32) -> Result<()> {
		if count == 0 {
			return Ok(());
		}
		let top = self.frame(0)?;
		if self.height == top.start {
			return if top.polymorphic { Ok(()) } else { Err("operand stack underflow".into()) };
		}
		self.height = self.height.checked_sub(count).ok_or("operand stack underflow")?;
		Ok(())
	}
}

fn numeric_type(ty: &wasmparser::ValType) -> bool {
	matches!(
		ty,
		wasmparser::ValType::I32 |
			wasmparser::ValType::I64 |
			wasmparser::ValType::F32 |
			wasmparser::ValType::F64
	)
}

// Use the maintained parser's declared MVP arities, not a duplicate opcode table.
fn fixed_arity(operator: &Operator<'_>) -> Option<(u32, u32)> {
	macro_rules! arities {
		($( @$proposal:ident $op:ident $({ $($arg:ident: $argty:ty),* })? => $visit:ident ($($ann:tt)*))*) => {
			match operator {
				$(Operator::$op $({ $($arg: _),* })? => arities!(@one $proposal $($ann)*),)*
				_ => None,
			}
		};
		(@one mvp arity $pop:literal -> $push:literal) => { Some(($pop, $push)) };
		(@one sign_extension arity $pop:literal -> $push:literal) => { Some(($pop, $push)) };
		(@one $($other:tt)*) => { None };
	}
	wasmparser::for_each_operator!(arities)
}

fn maximum_height(
	body: &wasmparser::FunctionBody<'_>,
	results: u32,
	module: &Meter,
) -> Result<u32> {
	// These rules intentionally preserve the historical estimator, including its
	// treatment of Else and BrTable. Changing them would change deterministic traps.
	let mut stack = Stack {
		height: 2,
		frames: vec![Frame { polymorphic: false, end: results, branch: results, start: 0 }],
	};
	let mut maximum = 0;
	for operator in body.get_operators_reader().map_err(|e| e.to_string())? {
		let operator = operator.map_err(|e| e.to_string())?;
		if !stack.frame(0)?.polymorphic {
			maximum = maximum.max(stack.height);
		}
		match operator {
			Operator::Block { blockty } | Operator::Loop { blockty } | Operator::If { blockty } => {
				let end = match blockty {
					BlockType::Empty => 0,
					BlockType::Type(
						wasmparser::ValType::I32 |
						wasmparser::ValType::I64 |
						wasmparser::ValType::F32 |
						wasmparser::ValType::F64,
					) => 1,
					_ => return Err("unsupported block signature in stack metering".into()),
				};
				let branch = if matches!(operator, Operator::Loop { .. }) { 0 } else { end };
				if matches!(operator, Operator::If { .. }) {
					stack.pop(1)?;
				}
				stack
					.frames
					.push(Frame { polymorphic: false, end, branch, start: stack.height });
			},
			Operator::Else | Operator::Nop => {},
			Operator::End => {
				let frame = stack.frames.pop().ok_or("empty control stack")?;
				stack.height = frame.start;
				stack.push(frame.end)?;
			},
			Operator::Unreachable => stack.unreachable()?,
			Operator::Br { relative_depth } => {
				stack.pop(stack.frame(relative_depth)?.branch)?;
				stack.unreachable()?;
			},
			Operator::BrIf { relative_depth } => {
				let arity = stack.frame(relative_depth)?.branch;
				stack.pop(arity)?;
				stack.pop(1)?;
				stack.push(arity)?;
			},
			Operator::BrTable { targets } => {
				let arity = stack.frame(targets.default())?.branch;
				for target in targets.targets() {
					if stack.frame(target.map_err(|e| e.to_string())?)?.branch != arity {
						return Err("branch table arities differ".into());
					}
				}
				stack.pop(arity)?;
				stack.unreachable()?;
			},
			Operator::Return => {
				stack.pop(results)?;
				stack.unreachable()?;
			},
			Operator::Call { function_index } => {
				let ty = module.function_type(function_index)?;
				stack.pop(ty.params().len() as u32)?;
				stack.push(ty.results().len() as u32)?;
			},
			Operator::CallIndirect { type_index, .. } => {
				let ty = module
					.types
					.get(type_index as usize)
					.ok_or("indirect call type out of bounds")?;
				stack.pop(1)?;
				stack.pop(ty.params().len() as u32)?;
				stack.push(ty.results().len() as u32)?;
			},
			Operator::Select => {
				stack.pop(2)?;
				stack.pop(1)?;
				stack.push(1)?;
			},
			_ => {
				let (pop, push) = fixed_arity(&operator)
					.ok_or_else(|| format!("unsupported stack-metering operator: {operator:?}"))?;
				stack.pop(pop)?;
				stack.push(push)?;
			},
		}
	}
	if !stack.frames.is_empty() {
		return Err("unterminated function body".into());
	}
	Ok(maximum)
}

#[cfg(test)]
mod tests {
	use super::*;
	use wasm_encoder::{
		ConstExpr, GlobalType, ImportSection, Module, TagKind, TagSection, TagType, ValType,
	};

	#[test]
	fn historical_stack_costs_and_thunks_match_fixed_modules() {
		macro_rules! fixture {
			($name:literal) => {
				(
					include_bytes!(concat!("stack_metering_fixtures/", $name, ".input.wasm"))
						.as_slice(),
					include_bytes!(concat!("stack_metering_fixtures/", $name, ".expected.wasm"))
						.as_slice(),
				)
			};
		}
		// Expected modules were generated by unmodified wasm-instrument 0.4.0.
		for (input, expected) in [
			fixture!("empty"),
			fixture!("import-export"),
			fixture!("locals"),
			fixture!("if-result"),
			fixture!("unreachable-else"),
			fixture!("recursive"),
			fixture!("br-table"),
			fixture!("br-if-loop"),
			fixture!("return-unreachable"),
			fixture!("unreachable-select"),
			fixture!("table-start"),
			fixture!("host-global"),
			fixture!("duplicate-type"),
			fixture!("float-memory"),
			fixture!("float-comparison"),
		] {
			assert_eq!(inject(input, 16).unwrap(), expected);
		}
		assert_eq!(
			inject(include_bytes!("stack_metering_fixtures/recursive.input.wasm"), u32::MAX)
				.unwrap(),
			include_bytes!("stack_metering_fixtures/recursive.max-limit.expected.wasm"),
		);
	}

	#[test]
	fn imported_globals_fail_instead_of_misdirecting_stack_counter_writes() {
		let mut module = Module::new();
		let mut imports = ImportSection::new();
		imports.import(
			"env",
			"global",
			wasm_encoder::EntityType::Global(GlobalType {
				val_type: ValType::I32,
				mutable: true,
				shared: false,
			}),
		);
		module.section(&imports);
		assert!(inject(&module.finish(), 16).unwrap_err().contains("imported globals"));
	}

	#[test]
	fn exception_tags_fail_before_inserting_sections_in_the_wrong_order() {
		let mut module = Module::new();
		let mut types = wasm_encoder::TypeSection::new();
		types.ty().function([], []);
		module.section(&types);
		let mut tags = TagSection::new();
		tags.tag(TagType { kind: TagKind::Exception, func_type_idx: 0 });
		module.section(&tags);
		assert!(inject(&module.finish(), 16).unwrap_err().contains("exception tags"));
	}

	#[test]
	fn custom_payloads_remain_opaque_and_keep_the_historical_reverse_order() {
		let mut module = Module::new();
		for data in [&[255, 255, 255][..], &[1, 2, 3][..]] {
			module.section(&wasm_encoder::CustomSection { name: "name".into(), data: data.into() });
		}
		let output = inject(&module.finish(), 16).unwrap();
		let custom: Vec<_> = Parser::new(0)
			.parse_all(&output)
			.filter_map(|payload| match payload.unwrap() {
				Payload::CustomSection(section) => {
					Some((section.name().to_owned(), section.data().to_vec()))
				},
				_ => None,
			})
			.collect();
		assert_eq!(custom, [("name".into(), vec![1, 2, 3]), ("name".into(), vec![255, 255, 255])]);
	}

	#[test]
	fn invalid_callees_and_local_count_overflow_return_errors() {
		for (locals, instruction) in [
			(vec![], I::Call(u32::MAX)),
			(vec![(u32::MAX, ValType::I32), (1, ValType::I32)], I::Nop),
		] {
			let mut module = Module::new();
			let mut types = wasm_encoder::TypeSection::new();
			types.ty().function([], []);
			module.section(&types);
			let mut functions = wasm_encoder::FunctionSection::new();
			functions.function(0);
			module.section(&functions);
			let mut code = wasm_encoder::CodeSection::new();
			let mut function = wasm_encoder::Function::new(locals);
			function.instruction(&instruction).instruction(&I::End);
			code.function(&function);
			module.section(&code);
			assert!(inject(&module.finish(), 16).is_err());
		}
	}

	#[test]
	fn counter_is_appended_after_existing_globals() {
		let mut module = Module::new();
		let mut globals = wasm_encoder::GlobalSection::new();
		globals.global(
			GlobalType { val_type: ValType::I32, mutable: true, shared: false },
			&ConstExpr::i32_const(99),
		);
		module.section(&globals);
		let output = inject(&module.finish(), 16).unwrap();
		for payload in Parser::new(0).parse_all(&output) {
			if let Payload::GlobalSection(section) = payload.unwrap() {
				assert_eq!(section.count(), 2);
				let mut globals = section.into_iter();
				for expected in [99, 0] {
					assert!(
						matches!(globals.next().unwrap().unwrap().init_expr.get_operators_reader().read().unwrap(), Operator::I32Const { value } if value == expected)
					);
				}
			}
		}
	}
}
