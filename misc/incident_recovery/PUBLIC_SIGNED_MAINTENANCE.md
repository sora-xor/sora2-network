# SORA Nexus public signed maintenance rehearsal

This opt-in source candidate enables real signed ordinary staking transactions after the exact recovery run activates runtime 138. It does not clear `maintenanceEpochLivenessAndStaking`: BABE configuration transitions, bounded maintenance intervals, subsequent elections/rewards and authenticated heartbeat coverage remain separate required evidence.

The existing replay schemas and reports remain unchanged when `fixtureSignedTransactions` is absent. The new `sora-nexus-public-fixture-signed-maintenance-v1` schema is a linear 134→138 public fixture, requires the original native incoming recorder, disallows repair engines, fixture overrides and supplied-council actions, and takes a SHA-256-bound transaction-plan artifact. It requires one old→new mapping to one of the already published public fixture council keys, the exact public council inventory, and a valid replacement-key signature over the standard checkpoint ownership message. It never accepts a private key or an arbitrary call.

The plan schema is `sora-nexus-public-fixture-staking-transactions-v1`. It pins `policyCommitment`, `signerScheme` (`sora-nexus/PUBLIC-FIXTURE-ONLY/council/v1/`), an index in the sorted published key list, `replacementOwnershipSignature`, a finite `funding` list and ordered `transactions`. Funding rows bind an exact work index/commitment and positive minimum native notifier amount. Transaction rows bind step, expected nonce, positive minimum free balance and maximum fee, a typed call, exact dispatch error (or null), and full before/after staking ledger projections. Calls are limited to bond, bondExtra, unbond, rebond, withdrawUnbonded, setPayeeOwner, payoutStakers and payoutStakersByPage. Destinations of bond/set-payee default to the signing replacement owner.

Before each transaction, the harness verifies the actual final runtime version, Complete phase, nonce, funds and ledger. It signs the actual call/extra, spec/transaction versions, genesis and mortal parent hash; tips are zero. It queries the exact executing Wasm for its fee and rejects absent/insufficient unlocked funds or a fee above the committed bound. The real signed extrinsic is applied to the real authored block. A transaction validity failure aborts; an expected dispatch failure must still be included, increment the nonce and satisfy the full ledger postcondition. Existing independent `Core_execute_block` import and disk snapshot restoration cover that block.

At completion, every declared funding operation must have one exact accepted IncomingAudit in the same in-process opaque replay certificate, revalidated against the final policy and root. The audit must precede signed execution and name the replacement's XOR receipt. Native notifier amounts can be gross amounts; the actual unlocked-balance/fee checks establish spendability. No JSON report can construct this certificate. Every transaction is joined to its exact block and snapshot witness. All finality, operator and production readiness flags remain false.

## Required new fixture and execution

No funded artifact or successful execution is included in this source candidate. Existing mixed fixture accounts are raw hashes without known signing keys and its XOR mint is only one base unit. The checkpoint XOR factor (10^38) limits u128 substrate transfers to at most three local base units, below transaction fees. EVM fungible funding is wip-only; legacy Ethereum XOR is restricted. A separately reviewed authenticated public funding fixture, or a legitimately compiled funded mapping, must supply the new key without raw state injection or fee bypass. The new native-balance audit action handles mapped balances credited after their static baseline; its runtime/artifact cohort is a prerequisite for an overlapping-balance rehearsal.

Root-owned verification after integration (reuse the warm target, generous deadline):

```
SKIP_WASM_BUILD=1 cargo test --locked --offline -p incident-recovery --features bridge-fixture maintenance::signed_fixture::tests
SKIP_WASM_BUILD=1 cargo test --locked --offline -p incident-recovery --features bridge-fixture public_funding_projection_requires_opaque_final_identity_and_fixture_scope
SKIP_WASM_BUILD=1 cargo clippy --locked --offline -p incident-recovery --features bridge-fixture --all-targets -- -D warnings
incident-recovery rehearse-maintenance --manifest <new-public-fixture-manifest> --output <new-owned-output>
```

The unit tests use isolated synthetic storage only to test signature construction and admission; those tests are not rehearsal evidence. Real fixture execution may use only committed authenticated incoming work and actual signed block extrinsics. This candidate neither invokes offchain workers nor signs BABE/GRANDPA or online heartbeat messages.
