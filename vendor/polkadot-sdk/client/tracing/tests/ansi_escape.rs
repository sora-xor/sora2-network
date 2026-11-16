// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0
// SORA Nexus regression coverage for RUSTSEC-2025-0055.

use std::{env, process::Command};

#[test]
fn logger_escapes_untrusted_ansi() {
	// Run the real logger in its own process: it owns a global subscriber and
	// flushes its stderr buffer when the process exits.
	const CHILD: &str = "SORA_NEXUS_TRACING_ANSI_TEST_CHILD";
	if env::var_os(CHILD).is_some() {
		let mut logger = sc_tracing::logging::LoggerBuilder::new("info");
		logger.with_colors(false);
		logger.init().unwrap();
		log::warn!("{}", "untrusted message \x1b]0;injected-title\x07\x1b[2J");
		return;
	}

	let output = Command::new(env::current_exe().unwrap())
		.env_remove("RUST_LOG")
		.env(CHILD, "1")
		.args(["--exact", "logger_escapes_untrusted_ansi", "--nocapture"])
		.output()
		.unwrap();
	assert!(output.status.success(), "child logger test failed: {output:?}");
	let stderr = String::from_utf8(output.stderr).unwrap();
	assert!(stderr.contains("untrusted message"), "{stderr:?}");
	assert!(stderr.contains("injected-title"), "{stderr:?}");
	assert!(!stderr.contains('\x1b'), "raw terminal escape in log: {stderr:?}");
}
