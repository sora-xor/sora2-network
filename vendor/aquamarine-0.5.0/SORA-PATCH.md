# SORA Nexus dependency maintenance patch

Upstream: crates.io `aquamarine-0.5.0`.
Published crate SHA-256: `21cc1548309245035eb18aa7f0967da6bc65587005170c56e6ef2788a4cf3f4e`.

The original crate version is retained. Consuming dependencies are migrated;
this is not a renamed copy of an unmaintained dependency.

Removed unmaintained proc-macro-error dependency. Diagnostics now return
`Result`/`syn::Error` and emit compiler error tokens directly through maintained syn;
there is no panic-hook adapter or copied proc-macro-error implementation.
