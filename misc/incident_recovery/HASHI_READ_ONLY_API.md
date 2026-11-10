# SORA Nexus read-only HASHI witness admission

This layer prepares native signing payloads and checks a completed witness in a
discarded storage overlay. It does not load keys, sign, send transactions, contact
an external chain, authorize credits, or start ordinary bridge workers. The
implementation is wired into the current runtime and the deployed-source
bootstrap exporter. Native compilation and exact-artifact verification remain
separate gates; wiring is not evidence that they passed.

The two operations take only a SCALE parent header and a SCALE incoming
operation. They do not accept a caller-supplied policy, signature domain, runtime
version, phase, cursor, peer set, nonce, or verification flag. Preparation returns
the native signed payload for each immutable approval template. Admission runs
the existing incoming authentication and application routines with the completed
signatures. Both discard every storage write, including native failure markers.

`recovery_hashi_prepare_source` additionally accepts a bounded SCALE receipt
projection and exact native load bytes. It checks the existing source-record
commitment, authenticates the raw transaction actor and outer destination,
requires one canonical native Deposit, and calls the existing bridge event
parser and amount conversion. The complete original request must match before
payload preparation resolves replacement owners. Retained Pending loads must
have their original queue/submission/replay records and a known outer target;
automatic event imports use the native beneficiary/timepoint recipe and permit
routed outer calls while requiring a known emitting contract. Failed loads,
historical transfer kinds and already-prepared incoming identities remain
outside this narrow path. The source input never proves receipt completeness,
inclusion or finality; peers must establish those observations independently.

The original spec-134 / transaction-131 bootstrap is the only admitted version.
The policy comes from the executing runtime's compiled binding. Its commitment
must equal the original and active stored commitments, with an Applying phase,
zero repair counter, no failure, a persistent bridge halt and no transient
inbound context. The requested operation must match the current cursor and its
committed business hash. The existing child-two recovery anchor must already be
installed. This API cannot create an anchor or support first-child signing.

Before opening a transaction, the supplied header must match the actual parent
state root, System height, parent hash, genesis and exactly one expected lineage
marker. Header input is bounded to 16 KiB; incoming bytes use the existing
bounded decoder and chunk/approval/weight limits. The preview then installs only
the prospective child's height, parent and parent block-hash lookup. This lets
native extensions derive the signing payload for height P+1 rather than P. It
is not an execution of consensus, timestamp or economic block hooks and makes
no whole-block validity claim.

Preparation checks current native bridge peers and multisig membership, exact
call/extra templates, the native fee exemption, the account nonce and the mortal
era's known post-checkpoint birth hash. It uses `SignedPayload::new` and
`using_encoded`, including the native long-payload hashing rule. Admission calls
the same native incoming path used by recovery, including signature checking,
nonce consumption, request/replay records, owner resolution and issuance
journals. Replacement recipients are resolved only after authentication.

## Caller obligations

Adding methods to the existing EthBridge runtime API retains its identifier and
version so that the deployed spec-134 RuntimeVersion API inventory stays exact.
Consequently, ordinary API-version discovery is insufficient: a future caller
must authenticate the **exact release-bound executing Wasm hash** before invoking
these methods. The stored `:code` hash returned in a response is descriptive; a
spec-134 substitute executes different bytes. A response cannot authenticate its
own executor or consensus finality.

The host must obtain the parent from its verified finalized recovery client and
check its lineage, exact stored state and executing artifact. It must bound the
raw API arguments before decoding and run in an on-chain/read-only execution
context without network, transaction-pool or keystore capabilities. It must join
the returned operation, phase/cursor, nonce, anchor, parent/birth hashes and native
version to the immutable release and execution-witness context before asking its
own local peer key to sign. A mortal era encodes period/phase, not an absolute
window; the host must reject a different birth/death window from the pinned
execution plan even if a later native era cycle would otherwise be valid.

The API permits only the original policy. A stale nonce, expired execution
window or changed business template requires the separately authorized repair
path; the caller cannot rebase the work or rewrite its commitments. Local
append-only witness replay checks do not replace native nonce/replay protection.

## Side effects and source trust

The storage transaction is not, by itself, a guarantee about host side effects.
The admitted call decoder permits only the existing HASHI incoming transfer
import/finalize through the specified BridgeMultisig account. That path reaches
on-chain request bookkeeping, reserves/transfers/mints, events and recovery
journals. It does not invoke ordinary offchain hooks, HTTP, transaction-pool
submission, offchain indexing, auxiliary client writes or a keystore. The native
tests run without such extensions and check the offchain-index overlay remains
empty as well as comparing the complete parent storage root after success and
failure. A future host must preserve those capabilities and code-identity bounds;
this document does not claim storage rollback contains an arbitrary host call
or a native-process panic.

HASHI's existing peer trust model remains in force. Peers must independently
check the approved source connection, chain identity, frozen canonical header,
transaction/receipt association and existing confirmation rule before signing
an exact approved template. Provider-observed finalized heads and receipt/trie
proofs are distinct from Ethereum beacon-consensus authentication. This API
neither initializes a new beacon client nor treats a supplied JSON finality flag
as evidence. Responses always report external finality as unverified.

Native compilation, execution and exact-artifact rehearsal results must be
recorded separately. The current test definitions use only deterministic public
fixture keys. Tests of the shared inner path use their actual compiled runtime
version; they do not manufacture a production spec-134 admission result.

## Recorded verification

The following results predate the source-association extension. That extension
requires fresh native current/legacy and exact-artifact verification.

The current runtime passed all nine focused native tests, strict Clippy including
test targets, and a no_std check for `wasm32-unknown-unknown`. The source-guarded
receipts are in `artifacts/recovery-28191817/fixtures/hashi-readonly-api-verification-v3/`.
The bootstrap exporter also passed nine pure tests, including an actual local
export that retained the deployed lock bytes and recorded the copied overlay.

The legacy spec-134 native executable also passed all nine shared tests and both
export-only tests. These exercised the public API under spec 134 / transaction
131 and verified every deployed RuntimeVersion field and all 28 API entries.
Receipts and the executable hash are retained in
`artifacts/recovery-28191817/fixtures/legacy-hashi-api-native-execution-v2/`.
The legacy lock amendment added only the already locked `scale-info` dependency
to the bridge API; package identities, versions and sources remained unchanged.
The first controller attempt failed its input guard and remains recorded as a
failure. No legacy Wasm build, signing worker, real-key operation or production
activation is claimed by these native tests.
