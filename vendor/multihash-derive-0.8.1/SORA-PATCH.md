# SORA Nexus dependency maintenance patch

Upstream: crates.io `multihash-derive-0.8.1`.
Published crate SHA-256: `1d6d4752e6230d8ef7adf7bd5d8c4b1f6561c1014c5ba9a37445ccefe18aa1db`.

The original crate version is retained. Consuming dependencies are migrated;
this is not a renamed copy of an unmaintained dependency.

Removed unmaintained proc-macro-error dependency. Diagnostics now return
`Result`/`syn::Error` and emit compiler error tokens directly through maintained syn;
there is no panic-hook adapter or copied proc-macro-error implementation.

The 2026-10-25 compiler-warning cleanup removes the unused private `ParseError`
left behind by the diagnostic migration, and marks retained parser punctuation
and keyword fields as intentionally unused. Parsing still consumes and validates
the same tokens in the same order; generated code and diagnostics are unchanged.
