// Offline reproduction of an existing bug in @polkadot/api-derive 16.5.6.
// Assertions pin the observed incorrect behavior; see ../frontend-findings.md.
import assert from 'node:assert/strict';
import { of, firstValueFrom } from 'rxjs';
import { TypeRegistry } from '@polkadot/types';
import { _stakerRewards } from '@polkadot/api-derive/staking';
const registry = new TypeRegistry();
const bn = (v) => registry.createType('Balance', v);
const compact = (v) => registry.createType('Compact<Balance>', v);
const acct = (n) => registry.createType('AccountId', '0x' + n.repeat(64));
const validatorA = acct('1'), validatorB = acct('2'), nominator = acct('3');
const era = registry.createType('EraIndex', 989);
const ids = [validatorA, validatorB].map(x => x.toString());
const entry = () => ({ total: compact(100), own: compact(10), others: [{ who: nominator, value: compact(90) }] });
const api = { registry, derive: { staking: {
 queryMulti: (ids) => of(ids.map(id => ({
  accountId: registry.createType('AccountId', id), stashId: registry.createType('AccountId', id),
  claimedRewardsEras: registry.createType('Vec<u32>', validatorA.eq(id) ? [era] : []),
  stakingLedger: { legacyClaimedRewards: registry.createType('Vec<u32>') }
 }))),
 _stakerExposures: () => of([[{ era, isEmpty: false, isValidator: false, nominating: ids.map(validatorId => ({ validatorId })), validators: Object.fromEntries(ids.map(id => [id, entry()])) }]]),
 _stakerRewardsEras: () => of([
  [{ era, eraPoints: bn(2), validators: Object.fromEntries(ids.map(id => [id, bn(1)])) }],
  [{ era, validators: Object.fromEntries(ids.map(id => [id, { commission: { unwrap: () => bn(0) } }])) }],
  [{ era, eraReward: bn(1000) }]
 ])
} } };
const result = await firstValueFrom(_stakerRewards('hunt-mixed-claims', api)([nominator.toString()], [era]));
console.log(JSON.stringify({ validatorAFullyClaimed: true, validatorBUnclaimed: true, derivedIsClaimed: result[0][0].isClaimed, validatorRewardsRetained: Object.entries(result[0][0].validators).map(([id, v]) => ({id, value: v.value.toString()})), expectedPending: 450, wouldAppearInPayouts: result[0].some(r => !r.isClaimed) }, null, 2));

assert.equal(result[0][0].isClaimed, true);
assert.equal(result[0][0].validators[validatorB.toString()].value.toString(), '450');
assert.equal(result[0].some(r => !r.isClaimed), false);
