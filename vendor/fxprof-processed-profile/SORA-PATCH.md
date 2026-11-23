# SORA Nexus maintenance migration

Original consumer: `fxprof-processed-profile 0.6.0`, crates.io archive SHA-256 `27d12c0aed7f1e24276a241aadc4cb8ea9f83000f34bc062b7cc2d51e3b0fabd`. The package version identifies the original source; dependencies are actually replaced rather than relabeled or exempted from audit.

Replace fxhash with maintained rustc-hash 2.1.3, following upstream fxprof-processed-profile 0.8.1. The hasher only indexes internal profile tables; the public profile API is unchanged. Upstream reference archive SHA-256: 25234f20a3ec0a962a61770cfe39ecf03cb529a6e474ad8cff025ed497eda557. Reference: https://crates.io/crates/fxprof-processed-profile/0.8.1 .

Validation: all-feature compilation and both upstream profile-serialization integration tests pass.

The 2026-10-25 compiler-warning cleanup removes an unused private-module duplicate
`CounterHandle`. The exported, actively used `counters::CounterHandle` is retained
unchanged; profile serialization and public APIs are unaffected.
