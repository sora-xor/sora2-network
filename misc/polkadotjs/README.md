# SORA VAL payout fixes

The 4.8.11 runtime candidate restores reward discovery in unmodified public
Polkadot.js Apps. Completed-era VAL budgets are exposed through standard
`staking.erasValidatorReward`, and a bounded migration publishes existing
budgets still inside the staking claim window. The payout hook replaces native
rewards, so positive displayed amounts cannot mint XOR or increase XOR stake.
Actual VAL payments also emit the standard staking reward event. See
[`runtime-upgrade-4.8.11`](../../runtime-upgrade-4.8.11/README.md) for release
validation and governance activation. This requires a runtime upgrade.

## Earlier frontend-only workaround

The runtime fixes are implemented in this checkout. The accompanying
`restore-val-staking-payouts.patch` restores payout discovery in
[Polkadot.js Apps](https://github.com/polkadot-js/apps/tree/05aa55e844c492d516dba3c4b0bd1b9311f9aeb2)
at commit `05aa55e844c492d516dba3c4b0bd1b9311f9aeb2`.

The patch below is retained as an earlier frontend workaround. The 4.8.11
node/runtime correction does not require that patch to be published by the
Polkadot.js maintainers. The older validation snapshots below describe their
recorded runtime versions, not current mainnet deployment.

## Fixed behavior

- Apps reads `xorFee.valStakingEraReward` and displays rewards in VAL, while
  stakes, fees, and slashes retain their native denomination.
- Every exposure page and the validator's own stake are retained. Claims are
  filtered per validator and page, so one paid validator cannot hide another's
  unpaid rewards. The Own validators view subscribes to live claims and shows
  only the unpaid pages' value.
- Runtime VAL payments execute inside both staking payout calls. Direct calls,
  utility batches, and multisig dispatch use the same atomic payment path.
  Failed payments roll back claims and earlier payments, allowing retry.
- Payout declarations and actual weights include VAL work even when native
  staking rewards are zero. Automatic buyback accounting retains the routed
  LiquidityProxy estimate and any larger successful execution weight.
- With `wip` enabled, token fees carry a separate asset amount instead of a
  native imbalance. Referral reminting subtracts the exact amount already paid.

Apps keeps direct-call signing for compatibility with deployed runtime 130.
It queues up to 40 calls, oldest era first; each call pays the next unclaimed
page. The original 4.8.9 runtime fix kept `ErasValidatorReward` at zero. The
4.8.11 candidate publishes VAL there only with native minting disabled in both
payout and era-remainder paths.

## Review and verification

See [validation results](validation.md) and
[runtime changes](runtime-fixes.md). The small staking fork diff and its
provenance are in
[`vendor/polkadot-sdk/frame/staking/PATCHES.md`](../../vendor/polkadot-sdk/frame/staking/PATCHES.md).

The final read-only mainnet evidence is in
[final-mainnet-validation.json](final-mainnet-validation.json). The original
bug fixtures are retained under [bug-hunt](bug-hunt/frontend-findings.md).

## Apply the frontend patch

From a checkout of the pinned Apps commit:

```sh
git apply --check /path/to/restore-val-staking-payouts.patch
git apply /path/to/restore-val-staking-payouts.patch
yarn install --immutable
yarn polkadot-dev-run-test --env node packages/apps-config/src/api/spec/soraSubstrate/derives packages/apps-config/src/api/spec/soraSubstrate/stakingRewards packages/page-staking/src/Payouts/transactions packages/react-signer/src/directCall
yarn build:code
```

Until the hosted frontend releases the patch, Developer → Extrinsics on SORA
still exposes `staking.payoutStakers`. On deployed runtime 130, submit that
call directly with a validator stash and a completed, unclaimed era. Wrappers
must wait for the runtime upgrade. The caller need not be the validator.
