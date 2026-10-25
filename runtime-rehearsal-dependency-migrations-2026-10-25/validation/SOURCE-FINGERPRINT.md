# Source identity across lint, tests and Wasm builds

`source-fingerprint.py` inventories the current SORA Nexus checkout without changing the Git index. It records each file's SHA-256, Git blob identity, executable mode and size; deleted HEAD/index paths remain explicit tombstones. A stable aggregate excludes capture time and checkout location. The record also binds Git HEAD, index identity and the uncommitted source overlay.

The inventory includes complete source roots: Cargo configuration, CI/hooks, runtime, common, node, pallets, every vendor, utilities, scripts, housekeeping, integration/bridge tooling, native runtime-upgrade bindings and dependency-security tooling. Root Cargo workspace members and local path dependencies discover new crate roots automatically. Files within these roots are included even when Git ignores them, so new vendors, standalone Cargo locks and binary test/build assets cannot disappear merely because they are untracked or ignored. Named root build files and root scripts/configuration are included too. Literal Rust `include!`, `include_str!` and `include_bytes!` references retain assets outside those roots; extra dynamic inputs can be added with `--include path`.

Three aggregates serve different purposes:

- **source**: compiler/tooling source, manifests/locks, configuration and binary assets. Documentation embedded by a literal Rust include remains in this aggregate.
- **supporting**: ordinary documentation, licenses and named provenance records. Prose-only changes do not invalidate a source-only comparison.
- **tooling**: rehearsal code, package manifests/lock and known fixture inputs under this new rehearsal directory. Generated snapshots, candidates, reports and logs are excluded to prevent self-hash cycles.

Known build/cache directories are excluded before traversal. Credential/private-key/environment filenames, including every `sydneycreds` pathname, are excluded before opening or reading them. The recorder lists exclusions and refuses a literal Rust include into excluded credentials. External/unbound symlinks also fail instead of silently hiding unrecorded inputs. Historical release archives outside source roots are excluded unless explicitly included or referenced as a literal Rust compile input.

Capture a provisional record during preparation:

```sh
python3 runtime-rehearsal-dependency-migrations-2026-10-25/validation/source-fingerprint.py \
  --label preparation
```

After source changes are frozen and the build owner is ready, capture the lint start record:

```sh
python3 runtime-rehearsal-dependency-migrations-2026-10-25/validation/source-fingerprint.py \
  --final --label clippy-start \
  --output runtime-rehearsal-dependency-migrations-2026-10-25/validation/source-clippy-start.json
```

Check the same source at lint completion and immediately before/after the candidate build, using distinct output filenames for each gate:

```sh
python3 runtime-rehearsal-dependency-migrations-2026-10-25/validation/source-fingerprint.py \
  --check runtime-rehearsal-dependency-migrations-2026-10-25/validation/source-clippy-start.json \
  --source-only \
  --output runtime-rehearsal-dependency-migrations-2026-10-25/validation/source-clippy-end-check.json
```

Exit status is **0** for equality, **1** for a changed input and **2** for an invalid or unstable capture. Omitting `--source-only` also compares supporting files and rehearsal tooling. Even a source-only check binds the inventory policy, recorder bytes and Git HEAD. Index changes made by a user are recorded but are not treated as source changes when worktree contents stay identical; capture itself rejects a concurrent HEAD/index mutation. File identity/size/mtime/mode are checked again before completing capture, and an inventory change aborts the operation.

The default status is **provisional**. `--final` labels a deliberate source freeze; it does not imply that Clippy, tests or the build passed. Actual command exit codes, compiler/tool versions, feature flags, build environment and candidate SHA-256 must be recorded alongside these fingerprints. This is repository source lineage, not a claim of hermetic execution: generated `OUT_DIR` inputs come from their recorded generators, registry/Git dependencies are bound by Cargo locks, and ambient toolchains/environment are the build owner's separate evidence.

Offline regression fixtures use temporary repositories and cover ignored vendors/configuration, binary/mode changes, staged/unstaged deletions, cache/evidence exclusion, embedded documentation, separate tooling/prose aggregates, credential non-access, symlink binding, historical archive exclusion, policy/HEAD/recorder changes and preservation of the index:

```sh
python3 runtime-rehearsal-dependency-migrations-2026-10-25/validation/test-source-fingerprint.py -v
```
