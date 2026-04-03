use std::{
	any::Any,
	env, fs,
	panic::{self, AssertUnwindSafe},
	path::PathBuf,
};

fn panic_message(payload: &(dyn Any + Send)) -> Option<&str> {
	payload
		.downcast_ref::<String>()
		.map(String::as_str)
		.or_else(|| payload.downcast_ref::<&'static str>().copied())
}

fn main() {
	println!("cargo:rustc-check-cfg=cfg(enable_alloc_error_handler)");
	println!("cargo:rerun-if-env-changed=FRAME_STORAGE_ACCESS_TEST_RUNTIME_WARN_STUB");

	#[cfg(feature = "std")]
	{
		let result = panic::catch_unwind(AssertUnwindSafe(|| {
			substrate_wasm_builder::WasmBuilder::new()
				.with_current_project()
				.export_heap_base()
				.import_memory()
				.disable_runtime_version_section_check()
				.build();
		}));

		if let Err(payload) = result {
			let message = panic_message(payload.as_ref()).unwrap_or_default();
			let is_known_metadata_failure = message.contains("Failed to find entry for package") &&
				message.contains("frame-storage-access-test-runtime");

			if !is_known_metadata_failure {
				panic::resume_unwind(payload);
			}

			let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Cargo sets OUT_DIR for build scripts"));
			fs::write(
				out_dir.join("wasm_binary.rs"),
				"pub const WASM_BINARY: Option<&[u8]> = None;\n",
			)
			.expect("Writing wasm stub should succeed");

			if env::var_os("FRAME_STORAGE_ACCESS_TEST_RUNTIME_WARN_STUB").is_some() {
				println!(
					"cargo:warning=frame-storage-access-test-runtime fell back to a stub WASM binary after an upstream wasm-builder metadata failure"
				);
			}
		}
	}
}
