# SORA preimage patch

Upstream: https://github.com/paritytech/polkadot-sdk.git
Tag: `polkadot-stable2603-3`
Commit: `e3737178ec726cffe506c907263aaaa417893fd0`
Path: `substrate/frame/preimage`

Rust sources originate from the exact resolved SDK checkout. Cargo workspace
metadata, lint policy and dependencies are expanded with the same versions and
SDK tag for this standalone vendored crate. Apache-2.0 license is retained.

SORA changes: an ordinary signed replay of an already supplied requested preimage is rejected with the existing
`AlreadyNoted` error, so signed replay failures retain transaction fees. First
requested provision keeps its success refund. Manager conversion of an unrequested
preimage to a requested preimage, repeated internal StorePreimage notes, request counters, deposits, storage layouts and
call encodings are preserved. Regression tests cover first provision, replay,
multiple outstanding requests, manager conversion and subsequent re-provision.
