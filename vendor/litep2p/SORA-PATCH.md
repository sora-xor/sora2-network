# SORA Nexus litep2p migration

Source: crates.io `litep2p` **0.15.3**, as required by Polkadot SDK stable2609.
Published archive SHA-256:
`75a2c94e6b822a70bf38263b39c26ce3bffbdd78795d7da8de134d1f0cb3f7ea`.
The source, tests, examples, build script and license are the actual 0.15.3 release.
No previous 0.13 implementation is relabeled.

Upstream now incorporates the Hickory 0.26 resolver construction, fallible builder
and Google UDP/TCP fallback previously backported locally. The new opaque
`DnsInitError` API prevents leaking resolver error types to node consumers.
Those upstream changes supersede the corresponding local source patch.
The manifest retains the reviewed dependency floors **hickory-resolver 0.26.3**
and **uint 0.10.2**. All other upstream dependency requirements and features remain
unchanged. The updated uint removes deprecated macro/limit syntax without changing
Kademlia integer arithmetic.

The library uses the application's authoritative Cargo lock. Its development
lockfile and Cargo-generated registry bookkeeping are omitted. Upstream's optional
transport and development dependencies are not silently removed to obtain clean
audit output. Build and audit results are recorded separately.
