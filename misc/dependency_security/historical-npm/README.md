# Historical SORA Nexus dependency evidence

These files preserve the original runtime rehearsal and frontend reproduction dependency records before
the October 2026 security migration. Their names end in `.historical` so they
are not operational npm manifests or lockfiles. `manifest.json` records the
original runtime locations and SHA-256 hashes; frontend-repro/SHA256.json records the frontend originals. Release Wasm files, snapshots, and prior
rehearsal results are unchanged.

The original 4.8.9 graph used machine-specific package paths. A strict relocation
of that common path in an isolated audit copy exposed 106 packages and the Zod
advisory GHSA-mrhx-c7xc-m3w5, previously hidden by an empty dependency graph.
These old records are evidence, not a secure installation recipe.

Use the operational manifests beside the current validation scripts. They use
the reviewed Chopsticks consumer, rebuilt executor, rebuilt crypto and provider under vendor/. New
rehearsals report the new executor version and do not reuse the old temporary
dependency installation by default.
