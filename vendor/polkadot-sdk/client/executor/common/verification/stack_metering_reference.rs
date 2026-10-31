// Optional historical reference harness; never part of the production dependency graph.
use sora2_stack_metering_probe::inject;
use wasm_instrument::parity_wasm::elements::{deserialize_buffer, serialize, Module};
fn cases() -> Vec<(&'static str, String)> {
	vec![
("empty", "(module)".into()),
("import-export", "(module (import \"env\" \"host\" (func $h (param i32) (result i32))) (export \"h\" (func $h)))".into()),
("locals", "(module (func (export \"run\") (param i32 f64) (result i32) (local i64 f32 f64) local.get 0 i32.const 2 i32.add))".into()),
("if-result", "(module (func (export \"run\") (param i32) (result i32) local.get 0 if (result i32) i32.const 42 else i32.const 99 end))".into()),
("unreachable-else", "(module (func (export \"run\") (param i32) (result i32) local.get 0 if (result i32) unreachable else i32.const 1 i32.const 2 i32.const 3 i32.add i32.add end))".into()),
("recursive", "(module (func $f (export \"run\") (param i32) (result i32) local.get 0 if (result i32) local.get 0 i32.const 1 i32.sub call $f else i32.const 7 end))".into()),
("br-table", "(module (func (export \"run\") (param i32) (result i32) block (result i32) block (result i32) i32.const 9 local.get 0 br_table 0 1 end end))".into()),
("br-if-loop", "(module (func (export \"run\") (param i32) (result i32) block (result i32) loop i32.const 7 local.get 0 br_if 1 drop i32.const 0 local.set 0 end i32.const 8 end))".into()),
("return-unreachable", "(module (func (export \"run\") (result i32) i32.const 8 return block i32.const 1 i32.const 2 drop drop end unreachable))".into()),
("unreachable-select", "(module (func (export \"run\") (result i32) unreachable select))".into()),
("table-start", "(module (type $t (func (param i32) (result i32))) (table 2 funcref) (elem (i32.const 0) $f $g) (func $f (type $t) local.get 0 i32.const 1 i32.add) (func $g (type $t) local.get 0 i32.const 2 i32.add) (func $s i32.const 0 call $f drop) (start $s) (func (export \"run\") (param i32) (result i32) i32.const 9 local.get 0 call_indirect (type $t)))".into()),
("host-global", "(module (import \"env\" \"host\" (func $h (param i32) (result i32))) (global $g (mut i32) (i32.const 8)) (func (export \"run\") (param i32) (result i32) local.get 0 call $h global.get $g i32.add))".into()),
("duplicate-type", "(module (type (func (result i32))) (type $t (func (result i32))) (func (export \"run\") (type $t) i32.const 7))".into()),
("float-memory", "(module (memory (export \"memory\") 1) (func $f (param f64) (result f64) local.get 0 f64.sqrt f64.const 1.5 f64.add) (func (export \"run\") (result i32) i32.const 0 f64.const 9 call $f f64.store i32.const 0 f64.load f32.demote_f64 f32.abs i32.trunc_f32_s memory.size i32.add i32.const 0 memory.grow drop))".into()),
("float-comparison", "(module (func (export \"run\") (param f32 f64) (result i32) local.get 0 f32.neg f32.const 4.5 f32.max f32.nearest local.get 1 f64.ceil f64.const 3.0 f64.copysign f32.demote_f64 f32.le))".into()),
]
}
fn main() {
	if let Some(path) = std::env::args().nth(1) {
		let raw = std::fs::read(&path).unwrap();
		let original: Module = deserialize_buffer(&raw).unwrap();
		let legacy = wasm_instrument::inject_stack_limiter(original, 1024)
			.map(|m| serialize(m).unwrap())
			.unwrap();
		let replacement = inject(&raw, 1024).unwrap();
		wasmparser::validate(&legacy).unwrap();
		wasmparser::validate(&replacement).unwrap();
		let a: Module = deserialize_buffer(&legacy).unwrap();
		let b: Module = deserialize_buffer(&replacement).unwrap();
		assert!(a == b, "complete runtime transformed modules differ");
		println!(
			"PASS full runtime, input {} bytes; old {} bytes, new {} bytes; byte-identical {}",
			raw.len(),
			legacy.len(),
			replacement.len(),
			legacy == replacement
		);
		std::fs::write("runtime.old.wasm", legacy).unwrap();
		std::fs::write("runtime.new.wasm", replacement).unwrap();
		return;
	}
	let out = std::path::Path::new("outputs");
	std::fs::create_dir_all(out).unwrap();
	let mut count = 0;
	for (name, wat) in cases() {
		let raw = wat::parse_str(&wat).unwrap();
		wasmparser::validate(&raw).unwrap();
		std::fs::write(out.join(format!("{name}.input.wasm")), &raw).unwrap();
		for limit in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 16, 24, 32, 1024, u32::MAX] {
			let original: Module = deserialize_buffer(&raw).unwrap();
			let legacy = wasm_instrument::inject_stack_limiter(original, limit)
				.map(|m| serialize(m).unwrap());
			let replacement = inject(&raw, limit);
			assert_eq!(
				legacy.is_ok(),
				replacement.is_ok(),
				"{name}/{limit}: old={legacy:?} new={replacement:?}"
			);
			if let (Ok(a), Ok(b)) = (legacy, replacement) {
				wasmparser::validate(&a).unwrap();
				wasmparser::validate(&b).unwrap();
				let aa: Module = deserialize_buffer(&a).unwrap();
				let bb: Module = deserialize_buffer(&b).unwrap();
				assert_eq!(aa, bb, "{name}/{limit}");
				std::fs::write(out.join(format!("{name}-{limit}.old.wasm")), a).unwrap();
				std::fs::write(out.join(format!("{name}-{limit}.new.wasm")), b).unwrap();
			}
			count += 1;
		}
	}
	println!("PASS {count} complete transformed module comparisons");
}
