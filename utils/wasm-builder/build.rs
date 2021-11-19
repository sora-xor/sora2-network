use std::env;
use std::path::PathBuf;
use substrate_wasm_builder::WasmBuilder;

fn main() {
    WasmBuilder::new()
        .with_project(
            PathBuf::new()
                .join(env::var("CARGO_MANIFEST_DIR").unwrap())
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("runtime/Cargo.toml"),
        )
        .unwrap()
        .import_memory()
        .export_heap_base()
        .build();
}
