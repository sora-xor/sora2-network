# SORA Nexus stable2609 dependency migration

Upstream: `smoldot-light` 1.3.2, published crates.io archive.
Archive SHA-256: `f5ae693a7dec686bb80f97a78e42bf96aec196776dba8b2ab11b16445c7268f1`.
Upstream Git commit: `5fe9121f81a58454542ac69a44c4d73f00f30283`.

The full upstream source was rebased to the version required by stable2609 and
its tooling. Archive checksums were checked against the registry lockfile entries.
The reviewed lru 0.19.0 requirement and explicit hex alloc feature are retained.
No LRU call-site adaptation was required by the source merge.

The local text changes are reproducible using `stable2609-customizations.patch`
against the archive above. `stable2609-customizations.json` records the patch
replay result and per-file checksums. The root workspace lockfile governs resolution;
the archive’s package-local lockfile is omitted.

Validation of the updated source is recorded in the stable2609 migration report;
the older results below apply to their original package versions only.

## Previous review rationale

### Earlier dependency migration

Upstream: `smoldot-light` 0.17.2, published crates.io archive.
Archive SHA-256: `f1bba9e591716567d704a8252feeb2f1261a286e1e2cbdd4e49e9197c34a14e2`.
Upstream Git commit: `59ebd05e463ff90f04631ee930fd7bedfa7b2908`.

Migrate the lru consumer to maintained 0.19.0 APIs, removing vulnerable or unsound transitive versions.
The package version identifies the original consumer source; the dependency version is not relabeled.

Declare required `alloc` dependency features explicitly instead of relying on the surrounding workspace: hex formatting/decoding require allocation even in the no-default-features build.

Validation: isolated no-default-features build with smoldot and lru 0.19.0 passes; the migration needs no further LRU API adaptations beyond nonzero capacities.
