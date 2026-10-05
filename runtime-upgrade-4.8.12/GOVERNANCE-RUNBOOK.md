# SORA 4.8.12 governance runbook

This package contains six **unsigned review calls** for runtime spec **134**,
transaction version **131**. The final Wasm is 3,086,396 bytes:
`9cdc615875e0664304c50bfc09350660388e4c015543fbb4d827995af9ad9037` (SHA-256).

The exact `system.setCode` proposal is 3,086,402 bytes with hash
`0x2ed47f947903c423083bb8cc796e121d6eb5cb7dfd7de9bcb18127be7867d16a`. All six calls passed independent
offline decoding and binding to this Wasm. Finalized governance preflight block
**27,907,565** recorded council threshold **4** and technical committee
threshold **3**. Use `council-settings.json` for exact current call files,
lengths, weights and the unrequested-preimage deposit estimate. Obtain a fresh
signed-account fee/deposit quote before submission.

The finalized external queue at block **27,907,565** contains **4.8.11**. Settings record
`activationReady: false`; the guarded route rejects replacement of an occupied
queue. Resolve the existing proposal through normal governance, then refresh
finalized baseline/governance state. If 4.8.11 enacts, repeat metadata and exact
Wasm validation against its deployed runtime before regenerating this package.
No direct supersession alternative is provided.

## Operator readiness before activation

1. Install and fund dedicated sr25519 `keep` accounts for Kensetsu/Apollo
   maintenance. Exercise both workers sharing an account and their durable
   nonce queue. Avoid independent transaction submitters competing for that
   account. The source patch includes
   `pallets/kensetsu/FUNDED-KEEPER-ROLLOUT.md`; package preparation does not install
   keys or attest funding/rollout.
2. Switch BABE/GRANDPA equivocation reporters to signed funded submission.
   Legacy report call names/arguments remain available, but bare reports are
   rejected and automatic unsigned submission APIs return no transaction.
3. Confirm bridge client proof, nonce and recovery behavior. Registered peers
   need zero XOR for authenticated protocol operations, including accepted
   local failures. Invalid proofs and replay reject before execution. Generic
   and substrate inbound clients may retain valid bare submissions. Accepted
   failed or partially applied work consumes its protocol nonce; use recovery
   rather than replaying it. Arbitrary peer calls remain paid.
4. Review bounded pending/stored-call readiness. New pending multisig operations
   are capped at 128 per account and stored calls at 16,384 bytes. Legacy
   operation counts are grandfathered through additive tracking; the decode
   cap also applies to old calls. Inspect the complete backlog and arrange
   recovery for any oversized entry, which normal approval/execution rejects.
   Membership removal
   requires explicit weighted dispatch for stored calls. Numeric old deposit
   fields are not currency reserves; no incoming fee escrow/bond is introduced.
   The public readiness scan is bounded and is not complete backlog or operator
   attestation.
5. Prepare sponsor-grant onboarding for zero-XOR Iroha claimants. A volunteer
   sponsor authorizes its own funds with a cap/budget, expiry and at most three
   attempts; failed settlement consumes an attempt and retains a fee. Funded
   claimants can pay directly. No automatic sponsor exists.

These prerequisites and the occupied queue keep the package review-only until
fresh finalized checks and operator readiness are established.

## Verify and refresh evidence

Verify the final ZIP companion SHA-256 and extracted `SHA256SUMS`. From
`validation`, use the pinned dependency lock (`npm ci` if needed):

```sh
python3 verify-package.py
node verify-governance-calls.cjs
```

The read-only public endpoint is `wss://mof2.sora.org`. If deployed code/metadata
changes, capture and repeat validation against the new baseline before creating
calls:

```sh
node capture-public-state.cjs
node verify-wasm-compatibility.cjs
node rehearse-equivocation-fees.cjs
python3 verify-fee-events.py
node rehearse-fee-policy.cjs --synthetic-fixtures
node check-bridge-readiness.cjs
```

The synthetic option enables only recorded local fixtures. The rehearsals use
mock Wasm signatures and do not establish operator rollout or production
consensus authoring. Native real-signature checks remain required.

Refresh finalized governance state and regenerate calls/settings immediately
before use, even if deployed code/metadata is unchanged:

```sh
node prepare-package.cjs
node prepare-governance-calls.cjs
python3 prepare-council-settings.py
node verify-governance-calls.cjs
python3 make-manifest.py
python3 verify-package.py
```

Preparation scripts allow read-only RPC and never sign or submit transactions.
An occupied queue is recorded as a prerequisite and only the guarded review
route is encoded. A blacklisted candidate stops preparation. Refresh signed
payer balances/fees and actual motion close bounds separately.

## Normal governance sequence

1. Note the exact `set-code-call.hex` bytes as a preimage, using the final
   `preimage-note-call.hex`. Obtain a fresh signer fee/deposit quote. Successful
   first requested-preimage provision is refunded; an unrequested preimage may
   require its storage deposit. After 4.8.12 activation, signed repeated
   provision is charged.
2. Use the guarded council call named by `council-settings.json`. Its atomic
   `utility.batchAll([democracy.externalPropose(target),
   democracy.externalProposeMajority(target)])` refuses an occupied queue and
   rolls back on failure. Verify exact proposal hash/length and fresh membership.
3. Read the actual motion index from `council.Proposed`, cast explicit votes,
   and query stored motion, current membership, weight and length before close.
   Prepared close bounds are estimates; unset motion indices are intentional.
4. Confirm inner execution succeeded and `democracy.NextExternal` contains the
   exact final candidate. Then use the prepared technical committee fast-track
   call, obtain its motion index from events, collect votes and refresh bounds.
5. Read the actual referendum index from `democracy.Started`. Normal referendum
   approval and enactment must install the exact code; preparation performs none
   of these chain actions.
6. After enactment and block initialization, verify spec 134, transaction
   version 131, candidate code hash and the complete fee policy. Check funded
   reporter/keeper operation, free useful cancellations, zero-XOR authenticated
   bridge handling, pre-execution replay rejection and migration sponsorship.

The preceding 4.8.11 reward publication migration is included. From captured
spec 132, verify retained completed-era VAL budgets are published to standard
staking reward storage while claims/ledgers remain intact. Actual payouts
remain VAL.
