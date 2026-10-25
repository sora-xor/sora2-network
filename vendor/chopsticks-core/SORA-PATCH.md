# SORA Nexus Chopsticks schema migration

Source package: `@acala-network/chopsticks-core` 1.5.2, published archive verified
against its registry SHA-512 integrity value in UPSTREAM.json. TypeScript source
and license come from AcalaNetwork/chopsticks commit
`f6eab24db852e24c2c66e459600c3b9d68e2aa7b`, the package's published gitHead.

The dependency on Zod 3 is replaced with maintained Zod 4.6.7, fixing
[GHSA-mrhx-c7xc-m3w5](https://github.com/advisories/GHSA-mrhx-c7xc-m3w5).
The genesis storage schema now explicitly supplies both the record key and value
schemas, as Zod 4 requires. CommonJS and ESM runtime files are changed together;
the five Zod-dependent declaration files are regenerated with TypeScript 7.0.2.
The full source type-check also needs an explicit HexString assertion on a
Polkadot header hash returned by `toHex()`; that assertion changes no JavaScript.

The executor dependency points to `../chopsticks-executor`, which is rebuilt from
reviewed Rust source. The package keeps its upstream name and version; it does
not relabel an old Zod or executor implementation as patched.

Upstream release scripts and devDependencies are omitted from the installed
runtime manifest because they assume Acala's monorepo build layout. Source and
published distribution files remain available for review.

Six offline tests in tests/zod-migration.cjs cover CommonJS and ESM genesis
parsing, block-RPC option forwarding, rejection before block execution, set-head
lookup semantics, and the fixed oversized-array check. The latter verifies that
a 100,000-element input rejected by a maximum-length constraint produces one
issue without running any element validators. The full TypeScript source also
passes declaration generation with the new Zod types.
