# SORA 4.8.12 validation

Wasm SHA-256: `9cdc615875e0664304c50bfc09350660388e4c015543fbb4d827995af9ad9037` (3,086,396 bytes).
Source overlay tree: `a16e9e1ac195575c6008c69434999b2587b81d01`. The 11 affected native
suites passed **1,140 tests**, with zero failures and three existing ignored
tests; another **430** maintenance/common/inbound bridge tests passed. All
**28** new runtime fee-policy regressions passed. The exact packaged Wasm passed
report-fee, broader fee-policy and metadata/host-interface compatibility checks.

The offline verifier binds the final Wasm, source patch, logs, scripts, inputs,
unsigned call payloads and checksum manifest.

## Source, build and native checks

`validation/source-provenance.json` records a base Git commit and explicitly
uncommitted source overlay. `source.patch` must reconstruct its recorded tree
from that base; source-file hashes bind the authorized fee fixes, pinned SDK
patches, runtime/version changes and lockfile. No clean source commit is claimed
for a working-tree build. Native/Wasm build reports bind their exact logs and
source tree.

The affected native suites cover report proof processing and charged duplicate
reports; free useful order cancellation and paid empty/duplicate/mixed failures
with atomic rollback; Rewards authorization; requested-preimage first provision,
paid replay and manager/internal compatibility; funded keeper signing and shared
durable nonce allocation; sponsored zero-XOR migration and paid settlement
failure; and bridge authentication, progress, failed application, replay,
bounded pending operations and stored-call handling. Native signature tests use
real cryptography. The final 11-suite gate passed 1,140 tests, zero failures and three existing
ignored tests; `cargo fmt --all -- --check` passed. The separate 430-test gate
also passed without failures or ignored tests. The three retained ignores are
the two chameleon-pool swap tests and `reminting_for_sora_parliament_works`.

## Exact Wasm checks

`validation/snapshot.json` pins public finalized state, genesis, runtime code and
metadata. Its baseline is spec 132. Local Chopsticks execution overlays the
exact candidate and runs initialization/upgrade hooks plus a timestamp inherent.
It checks protected staking claim/ledger entries and native XOR issuance.

`validation/equivocation-fee-wasm-rehearsal.json` checks all four BABE/GRANDPA
report calls: positive retained fees for signed reports and duplicates, signed
wrapper fees, rejection of bare calls from External/Local/InBlock and direct
block application, and disabled automatic unsigned submission APIs.
`validation/fee-event-check.json` independently matches captured charges to
payer balance decreases, nonce increments, fee events, tips and duplicate errors.
All nine signed report cases retained a positive fee; all four bare report
methods rejected External/Local/InBlock admission and direct block application.
The independent event verifier matched all nine charges to balances.

`validation/fee-policy-wasm-rehearsal.json` additionally requires:

- Charged empty/no-op cancellation batches, unauthorized Rewards calls and
  requested-preimage replay; free first useful requested provision.
- Free useful single/batch cancellation followed by charged replay. When the
  bounded public-state search finds no open order, an explicitly synthetic
  base-asset funding fixture places an order through candidate Wasm on an
  existing public book. It verifies actual asset locking and unlocking as well
  as fees; funding, issuance and provider adjustments are recorded.
- Charged invalid signed Kensetsu/Apollo maintenance and rejected bare calls.
- Rejection of invalid generic/substrate inbound proofs. This does not claim
  that valid bare inbound commitments are rejected; they remain supported.
- An explicitly synthetic authenticated legacy peer/quorum with zero XOR:
  accepted remote-failure reporting remains free, advances its nonce and
  consumes request state. Changed-timepoint replay rejects before execution,
  without charging XOR or advancing the nonce.

The requested-preimage, sell-order funding and zero-XOR legacy peer fixtures
override local storage only. Every fixture key/value is recorded; these checks prove candidate behavior
under those fixtures, not execution of a live mainnet request. A bounded scan of
256 recent finalized blocks found no usable direct legacy peer call and does not
establish that no bridge activity exists. A separate bounded report-spam sample
cannot attribute historical spam to the account named by the user: unsigned
extrinsics have no signer, and block authors do not necessarily submit a block's
extrinsics.

Headers and transaction signatures use Chopsticks' mock signature host. Native
checks separately cover real signing/proof behavior. No local branch is
submitted. This rehearsal does not prove production consensus authoring,
operator key installation/funding, relayer rollout or governance enactment.
All additional checks passed: 16 signed cases (12 paid, four free), five
invalid bare cases, and changed-timepoint legacy protocol replay rejection.
Every signed fee event matched the payer balance change and nonce increment.
The pinned state had no open orders; successful cancellation was verified
using the disclosed candidate-placed synthetic sell order on a public book.

## Compatibility and readiness

The metadata comparison must preserve existing call indices/arguments, storage
encodings, signed extensions and runtime APIs while allowing the append-only
sponsorship and bounded-tracking additions. Spec advances to 134; transaction
version stays 131. Import/export signatures, local Wasm instantiation and memory
shape are checked against the pinned SDK executor. This is not an attestation of
every operator's heap configuration. All structural compatibility checks passed. Existing SCALE encodings,
host function signatures, exports and runtime API versions are unchanged;
all append-only metadata additions are retained for review.

`validation/bridge-readiness.json` is a bounded read-only view of public peers,
pending operations and stored call sizes. Its scan limitations are material;
it does not attest complete backlog enumeration or operator rollout. Registered
protocol peers require no XOR. New pending operations are bounded at 128 per
multisig account and stored calls at 16,384 bytes; additive tracking preserves
legacy operations without a full-backlog migration. Old numeric deposit fields
are not currency reserves. No incoming fee escrow/bond is introduced.

Grandfathering covers operation counts, not the 16,384-byte decode cap. Old
oversized calls remain stored but normal decode/approval execution rejects
them. The bounded sample cannot prove that every legacy entry fits; complete
size inspection and any required recovery remain activation prerequisites.
New admission allowances and 3× protocol weight reservations use conservative
engineering estimates over existing benchmark weights. These behavioral tests
do not establish freshly measured worst-case execution throughput.

## Governance evidence and remaining prerequisites

A fresh finalized preflight must check deployed code/metadata, memberships,
external queue, blacklist, preimage status and close bounds. Six unsigned calls
are decoded offline from the deployed metadata and matched to the final Wasm.
All six calls passed offline decoding at finalized preflight block
**27,907,565**. Council threshold is **4**, technical committee threshold
**3**, and the candidate is not blacklisted. The setCode proposal hash is
`0x2ed47f947903c423083bb8cc796e121d6eb5cb7dfd7de9bcb18127be7867d16a` (3,086,402 bytes).
The source-calculated unrequested-preimage deposit is
**10.421553333230449800 XOR**, additional to transaction fees;
this is not a signed-account fee quote. Exact bounds and payloads are recorded
in `council-settings.json` and the governance reports.

The known external queue contains 4.8.11. `activationReady: false` means these are
review artifacts; the guarded route preserves an occupied queue and no direct
supersession alternative is provided. Resolve the queue normally and refresh
baseline/preflight before activation. If 4.8.11 enacts, repeat metadata and
exact-Wasm checks against the new deployed baseline before regenerating calls.

Dedicated `keep` accounts must be installed/funded, consensus reporters switched
to signed funded submission, relayer recovery/proof handling checked, and
sponsor-based migration onboarding prepared where claimants have zero XOR.
Public inspection and local compilation do not establish this operator
readiness. No governance transaction, infrastructure operation or live report
has been signed or submitted by package preparation.
