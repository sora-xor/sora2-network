# SORA Nexus recovery: remaining evidence under the CSV-only policy

This is a source-grounded blocker inventory dated 2026-10-30. Individual capture
observations below do not waive release gates or certify a release or restart.
The human supplies the replacement CSV. Checkpoint operators and
council membership are derived; fresh keys, ownership signatures, operator
attestation files, refunds, frontend work and Soratopia inputs are outside this
restart policy. No user secrets or additional manual input are requested here.

The runtime already resolves incoming beneficiaries **after authenticating the
original payload**, with HASHI query authors resolved independently. Catch-up
consumption, credit or attacker exclusion, balance/issuance journals and request
or nonce changes share one transaction. Runtime 138 retains the bridge halt and
the permanent recipient resolver. These source mechanisms still need execution
against the actual release work and exact artifacts. See
`runtime/src/incident_recovery_incoming.rs`, its audit/balance/issuance modules,
`pallets/eth-bridge/src/recovery.rs` and `INCOMING_COVERAGE.md`.

| Work | Existing implementation | Remaining concrete work |
| --- | --- | --- |
| Checkpoint incoming obligations | `verify-incoming-coverage` reconstructs the checkpoint and joins all native HASHI transfer/load records, the declared external inventory and complete policy work. Completed transfers suppress repeat credit; a completed load alone does not. | Generate current CSV-bound reconciliation/work and rerun the verifier. Do not relabel the old empty-external-inventory fixture as production coverage. |
| Public external evidence | `capture_history.mjs` captures SORA Nexus headers/blocks. `evm/kernel.cjs` preserves the reviewed candidate's verification body for supplied full EVM transaction/receipt tries and actual Deposit logs. `capture_hashi_deposits.cjs` provides bounded, resumable provider observations. `incoming-history-composition-candidate-v1` joins consecutive supplied HASHI history segments to exact work. | Complete the captures and fixed release integration: preserve full histories/configuration epochs, bind frozen public heads and verify the source inventory in both directions. These tools do not establish finality, omitted-network absence or historical contract discovery. |
| Native HASHI event association | `verify-hashi-association` in `hashi_association.rs` uses the native parser over checkpoint state and compares the entire original request, including denomination/precision, asset, recipient, author and timepoint. | Run it on real source receipts and require current/legacy parser parity for those exact inputs. It does not rerun the receipt-trie verifier or authenticate a source report by trusting its flags. |
| Individual incoming authentication | Existing channel `MultiProof` verification and HASHI native peer quorum remain authoritative. | Obtain actual commitment proofs; for HASHI, automate recovery-era reauthorization with existing peer keys as described below. Public data cannot generate private-key signatures. |
| Outgoing cancellation, no refunds | `bridge_history.rs`, `bridge_pending.rs`, `bridge_signing.rs`, `bridge_hashi.rs` and `incident_recovery_outgoing.rs` preserve liabilities/signatures while installing permanent local tombstones. | Automatically derive complete current checkpoint reviews, including nonterminal outgoing, signing hashes, native HASHI records and anomalous retained history; execute exact worker, retry, cancellation and status-report rejection checks. |
| Release admission | `incoming_release.py` binds coverage inputs; `bridge_state_release.py` checks observed halt/nonce/ACK equations. | Implement the remaining fixed evidence joins and exact worker/queue checks. `bridgeReconciliation` is not cleared by source hashes or an empty inventory, and the packager currently keeps production readiness false. |

The retained local receipt at
`artifacts/recovery-28191817/fixtures/incoming-coverage-checkpoint-v3/complete-local-inventory.stdout.json`
reports 230,147 native request records, 85,147 transfers, 85,156 loads and nine
unconsumed records. Its companion evidence classifies those nine as unresolved
Failed loads, with no checkpoint Pending transfer/load. It declares **zero
external deliveries and zero incoming work**, and keeps external completeness,
finality, authentication and exact execution false. These are historical
diagnostic observations, not a fresh verification for the current CSV. The nine
failed loads require explicit preserved dispositions; unsupported legacy assets
or missing receipts cannot be silently revived or counted as processed.

Production incoming coverage currently supports HASHI (`EVMLegacy`),
Substrate/Parachain channel transfers and controls, and TON transfers. Generic
EVM FA delivery is WIP-only and has no production coverage adapter. That is a
source-capability list, **not proof of current network activity**. The authenticated
checkpoint inventory in
`fixtures/trusted-csv-bridge-source-registry-v1/generated/report.json` beneath
`artifacts/recovery-28191817/` records 840 configuration rows and five configured
or request-bearing identities: HASHI internal network 0, Kusama, Polkadot,
Liberland and TON Mainnet. All four channel-verifier peer rows are present.
HASHI internal network 0 is distinct from Ethereum chain ID 1; the public
configuration and observed RPC chain ID agree. Synthetic fixtures cannot
substitute for these records or establish present signing availability.
External Substrate and
TON exhaustive-history/finality capture adapters were not found in the inspected
production tooling. TON logical times must not be treated as consecutive block
numbers. Public identity reads succeeded for Ethereum, Liberland, both relay
chains and TON's official bootstrap configuration in
`fixtures/public-counterpart-identities-v1/`; raw replies are preserved, and
finality/incoming completeness remain unverified. Use the exact-integer TON
sidecar, not the lossy JavaScript rendering of its shard identifier.

Both published parachain archive routes failed the read-only connection probe
in `fixtures/public-parachain-identities-v1/`. Their provider is not established;
being an older company endpoint alone is not evidence of banned infrastructure.
The route-classification correction records the primary registry and archive
provenance. A relay's accessible RPC does not provide the parachain's missing
historical commitments. This gap must remain visible until source evidence is
obtained; a failed response is never an empty delivery inventory.

The first Ethereum capture in `fixtures/public-hashi-deposit-inventory-v1/`
verified chain/genesis, the fixed head and contract code, but its archive-log
request was refused with HTTP 403 because that provider requires a token.
No range page was committed. A separate official public dRPC route answered the
four-request capability probe in `fixtures/public-ethereum-archive-capability-v1/`
and matched the same frozen head. Its empty block 0..99 response establishes only
that narrow query's result, not complete archive coverage. The failed capture
remains intact; evidence from different providers must not be silently mixed.

A separately admitted public Tenderly capture subsequently completed all 526
range pages through frozen Ethereum block 26,298,768 for the three exact
checkpoint contracts. Its completion receipt is
`fixtures/public-hashi-deposit-inventory-tenderly-v2-resume-3/execution.json`.
The classification contains 30,938 Deposit locators, each in a distinct
transaction. This is complete provider-observed coverage for the configured
addresses and bound; it does not prove all historical contract addresses were
included, authenticate receipt tries, establish native request associations or
permit credits. The native checkpoint contains 85,147 transfer records, so
historical address/configuration scope still requires explicit reconciliation.

The read-only checkpoint join in `fixtures/hashi-checkpoint-deposit-join-v2/`
then matched 30,908 current-contract deposits to completed native transfers,
leaving 28 absent transfers and two unresolved loads. Seven more failed local
loads are absent from that contract inventory: the first receipt batch therefore
contains 37 unique transactions. This diagnostic records raw checkpoint keys,
value hashes and capture-page pins; it does not replace native coverage or receipt
verification. Two completed native migration records identify the intermediate
and current bridge contract, in addition to the original genesis contract.
Historical contract capture and native admissibility remain separate checks;
discovery alone does not authorize reviving a failed request.

The provisional CSV report
`artifacts/recovery-28191817/fixtures/user-mapping-provisional-2026-10-29-v1/row-2-selected/checkpoint-inventory-v3/inventory.json`
reports one
`EthBridge.Peers` row, four `MultisigVerifier.PeerKeys` rows and three
`BridgeMultisig.Accounts` rows, all with zero affected entries. This supports
retaining those peer identities for this CSV; it proves neither network names nor
peer process availability. Repeat this intersection when the CSV changes.

HASHI has a separate operational dependency. In
`runtime/src/incident_recovery_incoming.rs`, `checked_hashi_approval` requires:

- The exact original native `BridgeMultisig` import/finalize call for the request.
- A canonical mortal era, zero tip and a nonmaximum nonce equal to the signer's
  current native account nonce.
- `CHECKPOINT_NUMBER < era.birth(current_block) < current_block`.
- Successful native `UncheckedExtrinsic::check` against the current chain
  context, followed by native nonce validation and bridge multisig/quorum rules.

Thus signatures depend on an actual recovery birth-block hash and the executing
runtime's signed domain. An archived approval with a pre-checkpoint birth fails
the explicit height check; an abandoned-branch post-checkpoint approval uses a
different birth hash. Keeping the same peer keys does not make those signatures
valid on the recovery branch. A public receipt or an evidence hash cannot replace
them. `hashi_codec.py` already has `prepare`, `signing-payloads` and `assemble`
modes, but no reviewed production peer-signing orchestration was found.

Retain HASHI's existing peer-quorum/source-provider trust model. Each peer must
independently check the frozen source head and original receipt through its own
source connection, the existing confirmation/recheck rules and the exact
committed receipt/transaction-root evidence. This is not a claim of separate
Ethereum beacon-client verification. Initializing a new light client is not a
restart prerequisite; trusting a caller's `verified` flag is not a substitute
for the existing peer checks either.

The smallest path preserving existing authentication is to freeze exact templates
before restart, then have an operational coordinator observe an authenticated
recovery child, derive its native signing context/nonces, obtain signatures from
the existing peer processes, assemble only the committed templates and feed the
inherent. Signatures are excluded from the business commitment to avoid a
circular block hash; signer/call/extra templates remain committed. This needs a
tested operational signing path, including nonce drift, expiry and retry behavior,
and must not resume ordinary outbound workers or export peer private keys. It
is not something a CSV or read-only capture can accomplish by itself.

The prebuilt policy can accept those later signatures: both HASHI variants in
`IncomingOperation::commitment` exclude `approval.signature`, and
`mixed_work.rs` commits that operation hash rather than the raw signed envelope.
The regression `hashi_future_signatures_preserve_policy_but_business_fields_change_it`
covers this distinction. The node's inherent provider selects the actual
parent-hash file and the runtime checks the committed operation template. The
signature exclusion does **not** permit changing signer, call, nonce, mortal-era
encoding, original request, registries or source evidence.

There is still a packaging gap: `incoming_release.py` binds exact work-shard byte
lengths and SHA-256 hashes, which change when real signatures replace placeholders.
Do not overwrite or relabel those immutable package objects. A minimal coordinator
needs a separately recorded execution-witness bundle tied to the original release
and operation-template commitment. Its fixed verifier must prove that only the
signature fields changed, bind the observed recovery parent/birth/runtime context,
check the original planned nonce/era, execute native admission and record the exact
assembled envelope hash. The business policy and runtime artifacts remain fixed.
The isolated `hashi_execution_witness.py` helper now implements offline byte and
context consistency checks and exclusive local appends; see
`HASHI_EXECUTION_WITNESS.md`. It always leaves native admission, signature,
state-proof, finality and production-readiness claims false. The read-only runtime
preparation/admission layer is described in `HASHI_READ_ONLY_API.md`. Its current
native tests, strict Clippy, no-std check and original spec-134 native-executor
compatibility tests passed. A local live-client request pump is being verified;
see `HASHI_LIVE_CLIENT.md`. Those native tests do not establish exact live-client
Wasm execution or collect peer signatures. Package integration and operational
peer signing remain missing. These helpers do not
clear those gates or authorize a new staged-release workflow.

A pre-restart rehearsal cannot claim to contain production signatures over a
future actual recovery birth hash. Pre-restart exact-artifact template/authentication
tests and post-child native witness admission must be reported as distinct stages;
the latter must finish before submitting that incoming work. Any required package
gate that currently assumes all actual signed shards exist before first startup
needs an explicit reviewed staged-release design, not a skip or a fabricated
receipt. Signing later also cannot repair an expired era or changed nonce encoded
in an immutable template; that requires the existing authorized forward-repair
mechanism and its new evidence bindings.

If that operational path is unavailable, keep the affected work held and the
release gate blocked. Reusing an already prepared original request may reduce
the native calls but still requires the applicable recovery-era finalization
quorum. Any alternative receipt-proof authorization or changed signature domain
requires an independently reviewed design and implementation; none is authorized
or implemented by this document.

Outgoing quarantine is local. It creates no refund, erases no evidence and cannot
invalidate previously issued external withdrawal signatures. Complete counterpart
nonce/authorization reconciliation is necessary before reopening. The restart
evidence must record the retained exposure honestly, rather than claim external
invalidation or create an additional payout. Exact no-refund worker/status-report
tests remain required even while the persistent halt is set.

The smallest next implementation sequence is:

1. Rebuild native composition with the final source after the successful real-CSV
   composition described below. Its automatic bridge history, signing, HASHI
   and scheduler reviews include 2,049 exact incompatible-call cancellations.
   This does not clear the nine failed-load dispositions or establish external
   incoming completeness. Keep native complete-prefix and cross-record
   validation authoritative.
2. Extend the completed source-registry inventory and identity observations into
   resumable public captures for each configured source. Reuse the
   reviewed EVM kernel/composer rather than invent another receipt parser. Bind
   historical contract/configuration transitions and explain every held source.
3. Join extracted receipts to native association, original/effective recipients,
   attacker exclusions and already-consumed state, generating actual inventory,
   reconciliation, work and audits. All composed storage edits must precede
   typed bridge work; use the existing balance/issuance audit derivation.
4. Add and rehearse the existing-peer HASHI signing coordinator, or explicitly
   keep that blocker unresolved. Native nonce/quorum checks cannot be waived.
5. Run exact-artifact bridge and activation checks, then bind the fresh receipts
   through fixed package hooks. Retained source-v15/v71 fixtures are supporting
   evidence only, especially after retained-key and CSV policy changes.

The real-CSV composition completed in `trusted-csv-composition-v8b`, with
122,662 changed keys and all 1,632 required original offence preimages
independently matched to their report IDs and the checkpoint denomination.
The retained execution receipt reports unchanged inputs. A later formatting-only
change to the command entry point does not convert this earlier execution into
verification of a new executable. The generated work still needs rebuilding and
rehearsing with the final release source and CSV.

Governance migration includes the confirmed council seat, elections, direct
votes, delegations and their balance restrictions. The predecessor client
database reconstruction completed at block 28,191,816, with full retained
headers/bodies and the complete captured parent state compared after reopening.
It is preparation for consensus tests, not a full/warp synchronization result.

The earlier funded-staking fixture completed 1,993 executed blocks, including
64 runtime-138 blocks. Its final report validation remains failed: after sessions
advanced and CurrentEra became 7,811, ActiveEra was still 7,810. The report's
omitted empty prior-policy audit field was separately reconciled with native
serialization; that correction does not waive the active-era transition gate.
The failure was traced to a slot gap skipping both election snapshot windows;
the guarded native snapshot fallback passed its regression tests and strict
Clippy. The full native runtime suite passed 457 tests, with six opt-in tests
ignored; the production queued-root test was separately executed and passed.
Fresh exact-artifact activation remains required. This fixture also
predates the final retained-key CSV policy and cannot authorize the release.

Older ignored candidate READMEs still mention final ownership proofs, supplied
operator inventories and a separate human approval of source scope. They describe
their historical scope and must not become current prerequisites. No new human
review signature is needed merely to derive a source inventory; public evidence
and fixed verification must establish its claims. `HASHI_CODEC.md` still describes
an approved operator route for its trusted head: that operational context must be
supplied by verified coordinator output, not an extra manual artifact request.
The maintenance manifest field `freshGrandpa` is retained schema vocabulary and
now carries unchanged checkpoint authorities; it is not a fresh-key gate.

The referenced observations and tests do not authorize a service change or
restart. No production signature collection, release promotion, bridge
reopening or alternate authentication path is claimed here.
