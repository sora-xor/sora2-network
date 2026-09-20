// Offline reproduction of an existing bug in @polkadot/api-derive 16.5.6.
// Assertions pin the observed incorrect behavior; see ../frontend-findings.md.
import assert from 'node:assert/strict';
import { of, BehaviorSubject, firstValueFrom } from 'rxjs';
import { TypeRegistry } from '@polkadot/types';
import { queryMulti } from '@polkadot/api-derive/staking';
const registry = new TypeRegistry();
const validator = registry.createType('AccountId', '0x' + '1'.repeat(64));
const era = registry.createType('EraIndex', 989);
const indexes = new BehaviorSubject({ activeEra: registry.createType('EraIndex', 990) });
const ledger = new BehaviorSubject(registry.createType('Option<StakingLedger>', { stash: validator, total: 100, active: 100, unlocking: [], claimedRewards: [] }));
let claimedPages = [], entryReads = 0;
const api = { registry, consts: { staking: { historyDepth: registry.createType('u32', 2) } },
 derive: { session: { indexes: () => indexes } },
 query: { staking: {
  bonded: () => of(registry.createType('Option<AccountId>', validator)), ledger: () => ledger,
  claimedRewards: { entries: () => { entryReads++; return of([[{ args: [era, validator] }, registry.createType('Vec<u32>', claimedPages)]]); } },
  erasStakersOverview: { entries: () => of([[{ args: [era, validator] }, { isSome: true, unwrap: () => ({ pageCount: registry.createType('u32', 1) }) }]]) }
 } }
};
let current;
const sub = queryMulti('hunt-claim-refresh-sub', api)([validator], {withClaimedRewardsEras:true,withLedger:true}).subscribe(r => {current=r;});
const before = current[0].claimedRewardsEras.toJSON();
claimedPages=[0];
ledger.next(registry.createType('Option<StakingLedger>', { stash: validator, total: 101, active: 101, unlocking: [], claimedRewards: [] }));
const stillSubscribed = current[0].claimedRewardsEras.toJSON();
const fresh = await firstValueFrom(queryMulti('hunt-claim-refresh-fresh', api)([validator], {withClaimedRewardsEras:true,withLedger:true}));
console.log(JSON.stringify({before,afterClaimAndEvenLedgerEmission:stillSubscribed,freshQuery:fresh[0].claimedRewardsEras.toJSON(),entryReads},null,2));
sub.unsubscribe();

assert.deepEqual(before, []);
assert.deepEqual(stillSubscribed, []);
assert.deepEqual(fresh[0].claimedRewardsEras.toJSON(), [989]);
assert.equal(entryReads, 2);
