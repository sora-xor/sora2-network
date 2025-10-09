# SORA Network Substrate Framework

This repository contains the Polkadot Substrate framework codebase for the SORA network. The implementation is written in Rust and provides the runtime, pallets, and supporting infrastructure needed to operate SORA on the Substrate stack.

For production builds, run `cargo build --release --features include-real-files` to include the real file assets bundled for deployment.

## Testing Guidelines
- Add at least one unit test for every function, and expand coverage with additional tests whenever the logic has multiple branches or non-trivial edge cases.
