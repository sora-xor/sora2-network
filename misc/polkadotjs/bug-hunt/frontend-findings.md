> **Status: fixed in the current frontend patch.** The scripts below reproduce
> the old upstream behavior. The repaired derives have regression tests, live
> claimed-page subscriptions, and full Apps build validation; see
> [validation.md](../validation.md). Own validators now uses the same reactive
> page accounting as the stash view, including partial-payout amounts.

# Frontend payout bug hunt

The existing `restore-val-staking-payouts.patch` restores the VAL reward budget,
but three reproduced defects in the retained staking derives and their UI
integration still need to be addressed before release. These findings concern
reward discovery, displayed amounts, and claim refresh. This hunt did not edit
the upstream patch or submit any transaction.

## Versions and reproduction

- Apps base: [`polkadot-js/apps` at
  `05aa55e844c492d516dba3c4b0bd1b9311f9aeb2`](https://github.com/polkadot-js/apps/tree/05aa55e844c492d516dba3c4b0bd1b9311f9aeb2),
  with the existing SORA payout patch applied.
- Examined patch SHA-256:
  `c459ba8c7dd98778866bef04f0391ac8f13d18b8ab8c032d351007235465c2b2`.
- Reproduced using `@polkadot/api-derive` 16.5.6, `@polkadot/types` 16.5.6,
  and RxJS 7.8.2. Direct dependencies and the resolved dependency graph are
  pinned in [frontend-repro/package.json](frontend-repro/package.json) and
  [frontend-repro/package-lock.json](frontend-repro/package-lock.json).
- The scripts call the published derive functions with explicit storage
  fixtures. They run offline after dependency installation and require no
  wallet, credentials, or running node.

From the SORA repository root:

```sh
cd misc/polkadotjs/bug-hunt/frontend-repro
npm ci --ignore-scripts --no-audit --no-fund
npm test
```

Each script prints its input and observed result. Its assertions deliberately
pin the existing incorrect behavior: a successful run confirms the bug is
reproduced, not that payout behavior is correct. After implementing fixes,
convert the relevant fixtures into regression tests for the expected results
below. Dependency source paths and line numbers below refer to the installed
16.5.6 JavaScript packages; Apps paths refer to the pinned checkout.

## 1. Paged exposure loses stakers and uses the wrong denominator

Run `node paged-rewards.mjs`.

The fixture has a validator with own stake 10, a first page containing Alice's
stake 30, and a second page containing Bob's stake 60. Overview storage records
total stake 100 and two pages. The validator earns the whole era reward of 1000
units and charges zero commission. All claims are pending.

| Recipient | Expected reward | Derived reward |
| --- | ---: | ---: |
| Validator | 100 | 0 |
| Alice, page 0 | 300 | 0 |
| Bob, page 1 | 600 | 1000 |

In `@polkadot/api-derive/staking/erasExposure.js:27`, `mapStakersPaged`
assigns each page to the same `validators[validatorId]` key. Later pages
overwrite earlier ones. It does not incorporate overview `own` and `total`.
In `staking/stakerRewards.js:21`, reward calculation then falls back to the
surviving page's `pageTotal` as its denominator. Earlier-page nominators get
zero, and a validator's own stake is absent from the calculation.

Apps `packages/page-staking/src/Payouts/index.tsx:102` removes zero-value stash
rows. Thus this defect can hide genuine unpaid rewards, in addition to showing
an excessive estimate for the surviving page. The separate "Own validators"
view calculates the overall validator reward directly; it does not use this
individual-staker calculation.

A correction must combine page data with the overview's total and own stake,
retain stakers from every page, and calculate pending contributions using claim
status. The earlier validation note treating multi-page behavior solely as an
estimate limitation understated the impact.

## 2. One validator's claim suppresses another validator's pending reward

Run `node mixed-validator-claims.mjs`.

The fixture has one nominator backing validators A and B in the same era. Each
validator owes that nominator 450 units before claims. A is fully claimed in
the paged `claimedRewards` state; B is unpaid. The validator ledgers' legacy
claim lists and the nominator's own claim list are empty.

Expected: A's contribution is removed and B's 450 units remain payable.
Actual: both 450-unit contributions remain, but the whole era is marked
`isClaimed: true`. The UI's `rewards.some(r => !r.isClaimed)` condition is false,
and no payout is queued for B from that era.

`@polkadot/api-derive/staking/stakerRewards.js:90` combines the nominator's
`claimedRewardsEras` with each validator's legacy ledger, rather than checking
the queried validator's `claimedRewardsEras`. The claimed contribution
therefore survives removal. Lines 120–121 then take claim status from the first
validator and stop, applying its result to the entire era. Reversing the
validator order can leave an already-paid validator eligible for the queue.

Apps `packages/page-staking/src/Payouts/index.tsx:103` filters the resulting
claimed era from stash rows, and the patch's
`packages/page-staking/src/Payouts/transactions.ts:19` excludes it from pending
transactions. The correction must filter claims separately for each validator
before grouping the remaining contributions.

## 3. "Own validators" does not refresh claimed status after a payout

Run `node claim-refresh.mjs`.

The fixture subscribes to the public staking `queryMulti` derive with claim and
ledger information. Era 989 initially has an unclaimed page. After the fixture
changes its storage result to a claimed page, the existing subscription still
reports no claimed eras, even when a ledger update emits. A fresh query reports
`[989]` as expected.

| Observation | Claimed eras |
| --- | --- |
| Initial subscription | `[]` |
| Existing subscription after claim and ledger emission | `[]` |
| Fresh query after claim | `[989]` |

`@polkadot/api-derive/staking/query.js:126` obtains claims with `.entries()`.
`@polkadot/api/base/Decorate.js:720` implements that operation using
`queryStorageAt`, so the values are a snapshot rather than a storage
subscription. Ledger updates do not rerun the entries read.

Apps `packages/react-hooks/src/useOwnEraRewards.ts:121` skips its
payout-event refetch whenever `ownValidators` is populated. That view instead
uses the long-lived account data from `useOwnStashInfos`. Consequently a fully
claimed era can remain listed and be queued again, producing `AlreadyClaimed`
until an unrelated refresh, such as a session change or reload. A session
change matters because the account query also depends on `session.indexes`.

The correction needs a fresh claim-state read after payout events in validator
mode, or reactive claim queries for the relevant validator/era pairs. The
fixture verifies the derive snapshot behavior; the UI consequence is traced
through source, not asserted by a browser test.

## Live scope check and limits

[frontend-live-scope.json](frontend-live-scope.json) records a read-only check
on 2026-09-12 at finalized SORA block
`0x837e68e9c1d7cb293c2140d141d292fe376ac94d4f9b97bfb4b2e5dec6bd80ff`,
examining completed era 7635. All 25 validator overviews in that era had one
page. The sampled zero-commission validator had zero own stake, so its zero
derived self-reward was expected. Its sampled nominator had a positive derived
reward. Storage reads used the recorded block; the public derive call used the
live API shortly afterward.

This check does not establish that a live account experienced any of the three
fixture failures. In particular, it is not evidence of lost multi-page payouts
on mainnet. The fixtures demonstrate supported configurations that the current
frontend mishandles. No transaction was signed or submitted, no browser wallet
flow was exercised, and the full Apps build remains unverified.
