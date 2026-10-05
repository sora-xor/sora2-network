# SORA 4.8.12 fee policy

This release removes the identified repeatable fee bypasses while preserving
successful order cancellation and the bridge protocol exception. Registered
bridge peers need no XOR for authenticated protocol operations, including
accepted operations whose local application fails. Invalid and replayed bridge
requests are rejected before execution. Ordinary signed calls secure payment
before dispatch; peer identity never makes unrelated calls free.

| Calls | Policy in 4.8.12 |
| --- | --- |
| BABE and GRANDPA equivocation reports, including the legacy `report_equivocation_unsigned` names | Signed funded payer; normal fees on success, invalid proof and duplicate report. Bare calls rejected. Existing proof verification and offence processing remain in use. |
| OrderBook single and batch cancellation | Successful cancellation refunded. Failure, replay, empty groups and empty batches charged. Group count and total order IDs are bounded. A failed atomic batch retains the orders. |
| Legacy EthBridge and BridgeMultisig | Registered peers' authenticated bridge protocol calls are free without XOR, including accepted protocol failures. Read-only admission checks reject invalid/replayed work before execution. Arbitrary peer calls and unrelated wrappers remain paid. The public user load-request call retains its existing fee. |
| Generic and substrate inbound channels | A verified fresh peer commitment is free, with either the existing bare submission or a signed submission. Invalid proof and replay are rejected before execution. Accepted local application failure remains free as part of the bridge exception; a partially applied batch retains its consumed nonce and successful items, preventing double credit. |
| Kensetsu accrue/liquidate and Apollo liquidation | Signed funded keeper; fees on success and failure. Bare admission rejected. Both workers use the dedicated `keep` key type and one durable nonce queue per account. |
| Iroha migration | A successful valid migration is refunded. Failed settlement remains paid. A zero-XOR claimant needs a voluntary claim-bound sponsor grant with a fee cap, budget, expiry and at most three attempts. Sponsor withdrawal and attempt consumption occur before dispatch and survive migration rollback. No sponsor is configured automatically. |
| Rewards UMI NFT receiver update (`add_umi_nft_receivers`) | Root success retains its exemption; a signed unauthorized call is charged. |
| Requested preimage provision | The first useful signed provision retains its refund. Repeated signed provision is rejected and charged. Internal preimage reference handling remains compatible. |

## Follow-up weight and sponsorship repair

Successful outgoing bridge approvals keep the full declared admission/validation
reservation instead of refunding it to the older processing-only weight. This
covers the fee-exemption checks as well as evidence processing before and at quorum.

A funded grant that cannot cover the current declared migration fee may be
replaced by a usable authorization for a zero-XOR claimant. Replacing a funded
grant cannot lower its existing maximum fee, remaining budget or attempts; the
normal sponsor authorization, funding and withdrawal rules remain in force.
The named Executive regressions cover these rules with real signatures.

## Remaining protocol exemptions

Timestamp and consensus inherents remain block-author operations. ImOnline
heartbeats remain authority-authenticated and bounded per session. Unsigned
election solutions retain their local/in-block admission and score/feasibility
checks. BridgeDataSigner approvals remain restricted to registered pending
hashes, authorized peers and unique signatures. BEEFy's public report calls are
not included in this mainnet runtime's call enum.

Existing conditional SDK refunds remain for useful governance, identity,
account/staking maintenance and verified beacon-header progress. They use their
existing authorization, consumed state or progress conditions. Test-only Faucet
and QaTools are excluded from the mainnet build. This inventory concerns the
resolved mainnet runtime; similarly named unused vendor copies are not evidence
that a call is publicly reachable.

## Bridge admission and bounds

The bridge exception is based on an authenticated, allowed protocol operation
and current request/nonce state. It is checked during transaction validation and
again before execution. The payment extension records an actual zero fee for
accepted exempt calls, including a local application failure. It does not
authorize free calls nested in arbitrary utility wrappers.

The mainnet configuration bounds new pending multisig operations per account
at 128 and stored call bytes at 16,384. Each proposer may open at most
`max(1, floor(128 / current multisig member count))` pending operations, so one
member cannot consume every current member's share. Existing proposals can
still receive approvals at either admission limit. Additive tracking preserves
existing operations without scanning the full legacy backlog; a legacy entry
without a proposer marker never decrements the new proposer counter. Membership removal no
longer dispatches an unbounded set of stored calls; explicit weighted dispatch
is required. Old numeric multisig `deposit` fields are not treated as currency
reserves. No incoming fee escrow or bond is introduced.

Current peers may vote directly with
`ethBridge.cancelPendingMultisig(networkId, callHash, timepoint)`. The current
multisig quorum releases the target's capacity without executing it; voters must
belong to both current membership sets. This lane bypasses creation limits and
does not need the original proposer, call bytes, or a still-fresh inner request.
A shared dispatch marker from another multisig does not prevent cleanup, and
cleanup preserves that marker.
Wrong timepoints, duplicate votes, removed voters and completed-operation
replays cannot receive a free successful execution. Normal dispatch and either
cancellation route clear pending cancellation votes and release marked counts
once. Cancellation does not reset incoming request status or a consumed
canonical transaction hash and does not retry a failed transfer.

Grandfathering applies to the pending-operation count. The 16,384-byte decode
limit also applies to old stored calls: oversized entries remain stored but
cannot be approved/executed through the normal decode path. The bounded public
sample fits this limit; complete size inspection and any necessary recovery
remain operator prerequisites.

Weight reservations combine existing benchmark weights with conservative
protocol multipliers and bounded admission allowances. They are engineering
estimates, not newly measured throughput benchmarks.

## Activation prerequisites

1. Install and fund dedicated `keep` accounts for maintenance workers. Test
   both pallets using the shared account without nonce replacement or gaps.
   Do not use consensus keys or an account simultaneously controlled by another
   transaction submitter. See `pallets/kensetsu/FUNDED-KEEPER-ROLLOUT.md` in the source checkout.
2. Preserve peer proof and channel nonce handling in inbound relayers. Bridge
   peers and inbound protocol relayers do not need XOR for exempt submissions.
   An accepted extrinsic can report a local application failure; consume its
   nonce and use the bridge's recovery workflow instead of replaying it.
3. Check finalized stored call sizes against the new limits and exercise
   zero-XOR approval, completion, failure and replay rejection. Existing pending
   operations are grandfathered and do not consume the new-operation limit.
   Exercise current-quorum cancellation of an abandoned proposal, including
   recovery while admission is full. The proposer quota applies to new entries;
   current quorum cleanup also handles old entries without proposer markers.
4. Switch consensus reporters to signed funded submission. The automatic
   unsigned report API returns no submitted transaction in this release.
5. Update zero-XOR migration onboarding to obtain a bounded sponsor grant.
   Sponsorship is optional for a claimant who can pay their own fees.
6. Resolve the existing external governance queue normally, repeat the
   finalized-state preflight and confirm client readiness before enactment.
   A compiled Wasm is not evidence that operators have completed these steps.

All calls in this package are unsigned review artifacts. Preparation does not
install keys, fund accounts, modify a remote service or submit a chain action.
