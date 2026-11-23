# SORA Nexus 4.8.16 runtime upgrade

Runtime spec **143**, transaction version **134**.

Hashi bridge workers now recognize the main bridge contract’s `Withdrawal(bytes32)` event from `receiveBySidechainAssetId` and `receiveByEthereumAssetAddress`. The event contains the original outgoing request hash. Once the Ethereum block has 30 confirmations, each worker checks the event against the successful transaction receipt and proposes `MarkAsDone` for an eligible outgoing transfer in `ApprovalsReady`. The bridge multisig records completion on the native chain.

Workers also check withdrawals below their existing live scan cursor. Each network and main bridge contract has its own persistent history position. One of every eight eligible network passes is available for that history. A history pass queries up to 4,096 blocks and checks at most one receipt. The configured Ethereum RPC must serve historical logs and receipts. The worker reduces the range after a provider limit and retries a failed page from its saved position. Scanning a page discovers candidates. Updating a request still requires an eligible transfer, a verified receipt and bridge quorum.

Each worker records its completed ranges locally. Retain the node’s offchain database when upgrading. Restoring an older database can repeat history work. Resetting the database loses the saved positions and subjects the live cursor to the node’s existing recovery bootstrap process.

## Adoption

Upgrade bridge nodes to the matching 4.8.16 binary before adopting spec 143 through the existing runtime upgrade process. The node checks the exact reviewed runtime code and genesis before granting worker key access or accepting local submissions. Existing guardian-halt acknowledgements, recovery checks and request quarantine continue to apply. Keep `--offchain-worker never` on nodes intended to have their workers disabled.

## Build inputs

Use `nightly-2026-10-23`, the checked-in Cargo lockfile and the verified external recovery policy at `runtime/src/incident_recovery/generated.rs` in a dedicated build checkout. Its SHA-256 is `b0a2c026fea1c33b524f8e0454881d5ab4cb94ae47a8d09fb59dc97ea3cb7a85`. Keep this generated file out of Git. The full native Ethereum bridge pallet suite passed all 471 tests.

The mainnet runtime was built from commit `014b9d9c5b4b0ae527f789a2c7189eb9044b6f86`. Its embedded runtime reports spec version 143 and transaction version 134. The compact Wasm has the same 55 import names and 88 export names as 4.8.15. Decompressing the upgrade payload reproduces the compact Wasm byte for byte.

| Release asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `framenode-runtime-4.8.16-spec143.compact.compressed.wasm` | 2,151,313 | `43290cf7ec2153a213a0f4524ec2b4067a04bb9e9ee90cd76b3711a81a3582c9` |
| `framenode-runtime-4.8.16-spec143.compact.wasm` | 16,431,903 | `3b02fde01bf5f9ab1ae3e1ecffb27d40af595068f7fb3407ab0c8ff92e330a31` |

The BLAKE2-256 code hash of the compressed upgrade payload is `0xc4633932066e1d36ce307c476f76ce8a9833f2c8bc9ea97837febf52ed90da68`.
