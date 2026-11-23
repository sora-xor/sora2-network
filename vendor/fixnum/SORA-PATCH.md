# SORA Nexus compiler maintenance

This existing vendored `fixnum` 0.6.0 implementation is retained. The 2026-10-25
compiler-warning cleanup removes an unused deprecated `core::i64` module import
and replaces `usize::max_value()` / `u64::max_value()` with their identical
associated `MAX` constants. Arithmetic, overflow checks, and public APIs are
unchanged. This note describes these narrow edits only; earlier vendored changes
and their dependency migrations remain intact.
