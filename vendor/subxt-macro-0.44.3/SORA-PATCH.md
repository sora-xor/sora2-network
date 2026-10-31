# SORA Nexus dependency maintenance patch

Upstream: crates.io `subxt-macro-0.44.3`.
Published crate SHA-256: `fc844e7877b6fe4a4013c5836a916dee4e58fc875b98ccc18b5996db34b575c3`.

The original crate version is retained. Consuming dependencies are migrated;
this is not a renamed copy of an unmaintained dependency.

Removed unmaintained proc-macro-error dependency. Diagnostics now return
`Result`/`syn::Error` and emit compiler error tokens directly through maintained syn;
there is no panic-hook adapter or copied proc-macro-error implementation.

The 2026-10-25 compiler-warning cleanup removes the retired
`no_mangle_generic_items` manifest lint. The compiler enforces this restriction
as a hard error; no active lint level or signing/code-generation behavior changes.
