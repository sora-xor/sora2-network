# SORA Nexus dependency migration

Upstream: `tracing-log` 0.2.0, published crates.io archive.
Archive SHA-256: `ee855f1f400bd0e5c02d150ae5de3840039a3f54b025156404e34c23c03f47c3`.
Upstream Git commit: `4161d8137d4f6117f17b110b0ec022d9350bf8e6`.

Migrate the lru consumer to maintained 0.19.0 APIs, removing vulnerable or unsound transitive versions.
The package version identifies the original consumer source; the dependency version is not relabeled.

The interest cache represents a disabled capacity as `None`, preserving zero-capacity behavior while using the new nonzero-capacity API and the same ahash hasher. Correct the upstream test mutex initializer to pass its unit value. Existing cache tests cover disabled state, eviction, and invalidation.

Migrate benchmark-only Criterion 0.3 to 0.8.2 as well: its old atty dependency has the unfixed RUSTSEC-2021-0145 unaligned-read warning. Benchmark and unit-test dependencies remain audited.

Restore the upstream benchmark’s path-only tracing-subscriber dev dependency (omitted during publication) as registry 0.3.23 with env-filter, so the retained benchmark can be checked.

For the 2026-10-25 full Clippy gate, remove the obsolete
`no_mangle_generic_items` lint name (the compiler now enforces a hard error) and
replace `private_in_public` with its active `private_bounds` and
`private_interfaces` lint names. No lint level is reduced and runtime logging
behavior is unchanged.

Interest-cache boolean tests use direct assertions of the same conditions;
coverage and cache behavior are unchanged.
