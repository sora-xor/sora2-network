# SORA Nexus memory-map dependency backport

Upstream: `parity-db 0.4.13`, published crate archive from
https://crates.io/crates/parity-db/0.4.13

Archive SHA-256 (verified against crates.io before extraction):
`592a28a24b09c9dc20ac8afaa6839abc417c720afe42c12e1e4a9d6aa2508d2e`

The archive records upstream Git commit
`8b5dba861dd787dd63aaeffe44c1061d1e6bb80f` at
https://github.com/paritytech/parity-db/.

The only dependency change is the normalized `Cargo.toml` requirement for
`memmap2`, from `0.5` to `0.9.12`. The initial migration retained upstream Rust sources, except for whitespace-only
formatter changes in `src/btree/mod.rs`, `src/file.rs`, and `src/log.rs`. Comment reflow is disabled
in the local rustfmt configuration to preserve upstream text with the pinned
compiler. `Cargo.toml.orig`, the changelog, and license files retain their
published contents. This preserves the database implementation and disk format;
it does not substitute the newer parity-db 0.5 API or storage implementation.

`memmap2 0.5.10` is affected by
[RUSTSEC-2026-0186](https://rustsec.org/advisories/RUSTSEC-2026-0186.html),
which fixes unchecked range offsets in memory-map advice and flush operations
in version 0.9.11. This dependency floor selects the current 0.9.12 release.
Upstream parity-db 0.5 also uses memmap2 0.9. The APIs used by this 0.4.13
source are retained by the newer dependency; the resolved application must still
pass compilation and database tests before release.

Existing database tests, including `test_indexed_keyvalues`,
`test_indexed_btree_1`, and `test_db_open_or_create`, cover writes, reads,
commits, and close/reopen behavior.
Run the vendored package's library tests through the root workspace and lockfile;
consult the repository dependency review for the actual validation result.

The 2026-10-25 full Clippy gate also updates redundant borrows, iterator calls,
Option predicates, and explicit elided lifetimes. Database lock/log file opens
now state `truncate(false)` explicitly, preserving the prior default and existing
file contents. Removing the final compression-field restoration is safe because
`TablesRef` is a by-value Copy view; its local copy is discarded immediately on
return, while the underlying data/table references and no-compression write
behavior are unchanged. Metadata and statistics loops retain the same order and
text. These edits do not change the database disk format or error policy.

The matching test-only cleanup removes redundant references and clones of Copy
values without changing fixtures or assertions.
