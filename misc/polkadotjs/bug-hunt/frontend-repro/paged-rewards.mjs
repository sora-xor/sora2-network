// Offline reproduction of an existing bug in @polkadot/api-derive 16.5.6.
// Assertions pin the observed incorrect behavior; see ../frontend-findings.md.
import assert from 'node:assert/strict';
import { of, firstValueFrom } from 'rxjs';
import { TypeRegistry } from '@polkadot/types';
import { _eraExposure, _stakerExposures, _stakerRewards } from '@polkadot/api-derive/staking';
const registry = new TypeRegistry();
const bn = (v) => registry.createType('Balance', v);
const compact = (v) => registry.createType('Compact<Balance>', v);
const acct = (n) => registry.createType('AccountId', '0x' + n.repeat(64));
const validator = acct('1'), alice = acct('2'), bob = acct('3');
const era = registry.createType('EraIndex', 989);
const page = (who, value) => ({ isSome: true, unwrap: () => ({ pageTotal: compact(value), others: [{ who, value: compact(value) }] }) });
const api = {
 registry,
 query: { staking: { erasStakersOverview: { entries: () => of([[{ args: [era, validator] }, { isSome: true, unwrap: () => ({ total: compact(100), own: compact(10), pageCount: registry.createType('u32', 2) }) }]]) }, erasStakersPaged: { entries: () => of([
   [{ args: [era, validator, 0] }, page(alice, 30)],
   [{ args: [era, validator, 1] }, page(bob, 60)]
 ]) } } },
 derive: { staking: {} }
};
const exposure = await firstValueFrom(_eraExposure('hunt-exposure', api)(era, true));
api.derive.staking._erasExposure = () => of([exposure]);
api.derive.staking._stakerExposures = _stakerExposures('hunt-stakers', api);
api.derive.staking.queryMulti = (ids) => of(ids.map(id => ({
 accountId: registry.createType('AccountId', id), stashId: registry.createType('AccountId', id),
 claimedRewardsEras: registry.createType('Vec<u32>'), stakingLedger: { legacyClaimedRewards: registry.createType('Vec<u32>') }
})));
api.derive.staking._stakerRewardsEras = () => of([
 [{ era, eraPoints: bn(1), validators: { [validator.toString()]: bn(1) } }],
 [{ era, validators: { [validator.toString()]: { commission: { unwrap: () => bn(0) } } } }],
 [{ era, eraReward: bn(1000) }]
]);
const rewards = await firstValueFrom(_stakerRewards('hunt-rewards', api)([validator.toString(), alice.toString(), bob.toString()], [era]));
console.log(JSON.stringify({
 input: { eraBudget: 1000, validatorOwn: 10, page0Alice: 30, page1Bob: 60, commission: 0 },
 expected: { validator: 100, alice: 300, bob: 600 },
 actual: Object.fromEntries(rewards.map((rs, i) => [['validator', 'alice', 'bob'][i], rs[0]?.validators[validator.toString()]?.value.toString()])),
 exposureSurvivingPage: exposure.validators[validator.toString()].pageTotal.toString()
}, null, 2));

assert.deepEqual(rewards.map(rs => rs[0].validators[validator.toString()].value.toString()), ['0', '0', '1000']);
assert.equal(exposure.validators[validator.toString()].pageTotal.toString(), '60');
