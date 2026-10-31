'use strict';
// Exact candidate Wasm only. Synthetic staking/account writes stay on local branches.
const assert = require('node:assert/strict');
const { GenericExtrinsic } = require('@polkadot/types');
const { hexToU8a, u8aToHex } = require('@polkadot/util');

async function payoutBatchFixture(block, context) {
  const { chain, Block, report, rawQuery, storageKey, signedCheck, signedExtrinsic, applyExtrinsic,
    progress, save, syntheticFixtures } = context;
  assert(syntheticFixtures, 'Payout batch evidence requires --synthetic-fixtures');
  const meta = await block.meta;
  const registry = meta.registry;
  // The JS package's legacy FeeDetails alias omits the trailing SCALE tip.
  registry.register({ PayoutRehearsalFeeDetails: { inclusionFee: 'Option<InclusionFee>', tip: 'Balance' } });
  const unit = 10n ** 18n;
  const minimum = 70_000_000_000_000n;
  const divisor = 100n;
  const multiplier = (await rawQuery(block, 'xorFee', 'multiplier')).toBigInt();
  const nextMultiplier = (await rawQuery(block, 'transactionPayment', 'nextFeeMultiplier')).toBigInt();
  const payer = registry.createType('AccountId', '0x' + '8a'.repeat(32));
  const validators = [0x8b, 0x8c, 0x8d].map(byte => registry.createType('AccountId', '0x' + byte.toString(16).repeat(32)));
  const nominators = Array.from({ length: 16 }, (_, index) =>
    registry.createType('AccountId', '0x' + (0xa0 + index).toString(16).repeat(32)));
  const val = '0x0200040000000000000000000000000000000000000000000000000000000000';
  const era = (await rawQuery(block, 'staking', 'currentEra')).unwrap().toNumber() - 1;
  const previousSigner = report.signer;
  const evidence = report.payoutBatchFees = {
    passed: false, synthetic: true, divisor: divisor.toString(),
    multiplier: multiplier.toString(), nextFeeMultiplier: nextMultiplier.toString(),
    minimumPerPayoutBeforeMultiplier: minimum.toString(), cases: [], safeguards: {},
    limitations: [
      'Synthetic validators, exposure pages and VAL budgets exercise candidate Wasm; they do not describe live validator rewards.',
      'Ordinary-fee comparisons apply the candidate transaction-payment weight/length conversion and the pinned fee multipliers to the same observed weights. They are counterfactual prices, not a replay under another deployed runtime.',
      'Each case has isolated synthetic block accounting; the fixture does not claim all cases fit in one block.',
      'Transaction signatures use the mock host. Native tests cover real signatures and nonzero tips.',
    ],
  };
  const encode = value => u8aToHex(value.toU8a());
  const clone = async (parent, label, rows) => {
    const writes = [];
    for (const [pallet, name, params, value] of rows) {
      const key = storageKey(meta, pallet, name, params);
      const encoded = value === null ? null : encode(registry.createType(key.outputType, value));
      writes.push([key.toHex(), encoded]);
      report.fixtureOverrides.push({ kind: 'payoutBatchFees', fixtureOnly: true, label,
        pallet, name, key: key.toHex(), value: encoded, parentStateBefore: await parent.get(key.toHex()) ?? null });
    }
    const child = new Block(chain, parent.number, parent.hash, parent, {
      header: await parent.header, extrinsics: [], storage: parent.storage,
    });
    child.pushStorageLayer().setAll(writes);
    return child;
  };
  const clean = (parent, label) => clone(parent, label + ' isolated block accounting', [
    ['system', 'blockWeight', [], null], ['system', 'blockSize', [], null],
  ]);
  const weightFee = async (parent, weight) => {
    const response = await parent.call('TransactionPaymentApi_query_weight_to_fee', [encode(registry.createType('Weight', weight))]);
    return registry.createType('Balance', hexToU8a(response.result)).toBigInt();
  };
  const lengthFee = async (parent, length) => {
    const response = await parent.call('TransactionPaymentApi_query_length_to_fee', [encode(registry.createType('u32', length))]);
    return registry.createType('Balance', hexToU8a(response.result)).toBigInt();
  };
  const ceil = value => value / divisor + (value % divisor === 0n ? 0n : 1n);
  const maximum = (left, right) => left > right ? left : right;
  const scaled = value => value * multiplier / unit;
  const baseWeight = meta.consts.system.blockWeights.perClass.normal.baseExtrinsic;
  const maxExtrinsic = meta.consts.system.blockWeights.perClass.normal.maxExtrinsic.unwrap();
  evidence.normalMaxExtrinsic = maxExtrinsic.toJSON();
  const fitsExtrinsic = weight => weight.refTime.toBigInt() <= maxExtrinsic.refTime.toBigInt()
    && weight.proofSize.toBigInt() <= maxExtrinsic.proofSize.toBigInt();
  const baseFee = await weightFee(block, baseWeight);
  const formula = async (parent, weight, length, count, discounted) => {
    const next = (await rawQuery(parent, 'transactionPayment', 'nextFeeMultiplier')).toBigInt();
    const adjustedWeight = (await weightFee(parent, weight)) * next / unit;
    const ordinary = scaled(baseFee) + scaled(adjustedWeight) + await lengthFee(parent, length);
    const custom = maximum(BigInt(count) * minimum, ceil(baseFee + adjustedWeight));
    const expected = discounted ? scaled(custom) + await lengthFee(parent, length) : ordinary;
    return { expected, ordinary, custom, adjustedWeight };
  };
  const quote = async (parent, call, count, discounted) => {
    const account = await rawQuery(parent, 'system', 'account', [payer]);
    const tx = signedExtrinsic(registry, call, parent, account);
    const params = [tx.toHex(), encode(registry.createType('u32', tx.encodedLength))];
    const info = registry.createType('RuntimeDispatchInfo', (await parent.call('TransactionPaymentApi_query_info', params)).result);
    const details = registry.createType('PayoutRehearsalFeeDetails', (await parent.call('TransactionPaymentApi_query_fee_details', params)).result);
    assert(details.inclusionFee.isSome, 'Paid payout fixture must expose inclusion fee details');
    const inclusion = details.inclusionFee.unwrap();
    const total = inclusion.baseFee.toBigInt() + inclusion.lenFee.toBigInt() + inclusion.adjustedWeightFee.toBigInt() + details.tip.toBigInt();
    assert.equal(total, info.partialFee.toBigInt(), 'query_info and query_fee_details must agree');
    assert.equal(inclusion.lenFee.toBigInt(), await lengthFee(parent, tx.encodedLength));
    const expected = await formula(parent, info.weight, tx.encodedLength, count, discounted);
    assert.equal(info.partialFee.toBigInt(), expected.expected, 'Payout eligibility must produce the exact fee formula');
    return { tx, info, ...expected };
  };
  const run = async (parent, label, call, count, discounted, success = true, error = null) => {
    const isolated = await clean(parent, label);
    const before = await quote(isolated, call, count, discounted);
    const after = await signedCheck(isolated, label, call, success, false, error);
    const row = report.signedChecks.at(-1);
    assert.equal(row.quotedFee, before.info.partialFee.toString());
    const systemEvent = row.events.find(event => event.section === 'system' && /^Extrinsic(Success|Failed)$/.test(event.method));
    assert(systemEvent, label + ' must expose post-dispatch weight');
    const dispatch = systemEvent.data.at(-1);
    // System includes base-extrinsic weight in the event; transaction-payment does not.
    const actualWeight = {
      refTime: (BigInt(dispatch.weight.refTime) - baseWeight.refTime.toBigInt()).toString(),
      proofSize: (BigInt(dispatch.weight.proofSize) - baseWeight.proofSize.toBigInt()).toString(),
    };
    const actual = await formula(isolated, actualWeight, before.tx.encodedLength, count, discounted);
    assert.equal(BigInt(row.xorCharged), actual.expected, label + ' actual debit must follow the actual-weight fee formula');
    assert(BigInt(row.xorCharged) <= before.info.partialFee.toBigInt(), label + ' settlement cannot exceed admission quote');
    assert(BigInt(actualWeight.refTime) <= before.info.weight.refTime.toBigInt());
    assert(BigInt(actualWeight.proofSize) <= before.info.weight.proofSize.toBigInt());
    const result = { label, payoutCount: count, discounted, passed: true,
      queryDetailsMatch: true, actualFeeEventMatchesBalance: row.actualFeeEventVerified,
      encodedLength: before.tx.encodedLength, declaredWeight: before.info.weight.toJSON(), actualWeight,
      quotedFee: row.quotedFee, xorCharged: row.xorCharged,
      ordinaryQuotedFeeCounterfactual: before.ordinary.toString(),
      ordinaryActualFeeCounterfactual: actual.ordinary.toString(),
      refunded: before.info.partialFee.toBigInt() > BigInt(row.xorCharged),
      rewardEvents: row.events.filter(event => event.section === 'xorFee' && event.method === 'ValStakingRewardPaid').length,
    };
    evidence.cases.push(result); save();
    return { block: after, row, result };
  };
  const reject = async (parent, label, call, reason) => {
    const isolated = await clean(parent, label);
    const account = await rawQuery(isolated, 'system', 'account', [payer]);
    const events = (await rawQuery(isolated, 'system', 'events')).toHex();
    const tx = signedExtrinsic(registry, call, isolated, account);
    const result = { label, expectedInvalidReason: reason, sources: {}, passed: false };
    evidence.cases.push(result);
    for (const source of ['External', 'Local', 'InBlock']) {
      const response = await isolated.call('TaggedTransactionQueue_validate_transaction',
        [registry.createType('TransactionSource', source).toHex(), tx.toHex(), isolated.hash]);
      const outcome = registry.createType('TransactionValidity', response.result);
      result.sources[source] = outcome.toJSON();
      assert(outcome.isErr && outcome.asErr.isInvalid && outcome.asErr.asInvalid.type === reason, label + ' ' + source);
    }
    const response = await applyExtrinsic(isolated, tx.toHex());
    const outcome = registry.createType('ApplyExtrinsicResult', response.result);
    result.apply = outcome.toJSON();
    assert(outcome.isErr && outcome.asErr.isInvalid && outcome.asErr.asInvalid.type === reason, label);
    const rejected = new Block(chain, isolated.number, isolated.hash, isolated, {
      header: await isolated.header, extrinsics: [], storage: isolated.storage,
      storageDiff: Object.fromEntries(response.storageDiff),
    });
    assert.equal((await rawQuery(rejected, 'system', 'account', [payer])).toHex(), account.toHex());
    assert.equal((await rawQuery(rejected, 'system', 'events')).toHex(), events);
    result.passed = true; result.balanceNonceAndEventsPreserved = true; save();
  };
  try {
    report.signer = { address: payer.toString(), synthetic: true, purpose: 'Payout batch exact-Wasm fee fixture' };
    const rows = [];
    let issuanceIncrease = 0n;
    for (const accountId of [payer, ...validators, ...nominators]) {
      const old = await rawQuery(block, 'system', 'account', [accountId]);
      assert.equal(old.data.free.toBigInt(), 0n, 'Synthetic staking account must not collide with funded public state');
      const account = old.toJSON();
      const funding = accountId.eq(payer) ? 10_000n * unit : unit;
      account.nonce = 0; account.providers = Number(account.providers) + 1; account.data.free = funding.toString();
      issuanceIncrease += funding;
      rows.push(['system', 'account', [accountId], account]);
    }
    rows.push(['balances', 'totalIssuance', [], ((await rawQuery(block, 'balances', 'totalIssuance')).toBigInt() + issuanceIncrease).toString()]);
    rows.push(['staking', 'erasValidatorReward', [era], '0']);
    rows.push(['staking', 'erasRewardPoints', [era], { total: 3, individual: new Map(validators.map(validator => [validator.toString(), 1])) }]);
    rows.push(['xorFee', 'valStakingEraReward', [era], (300n * unit).toString()]);
    for (const nominator of nominators) rows.push(['staking', 'payee', [nominator], 'Stash']);
    for (const validator of validators) {
      rows.push(
        ['staking', 'bonded', [validator], validator.toString()],
        ['staking', 'ledger', [validator], { stash: validator.toString(), total: '100', active: '100', unlocking: [], legacyClaimedRewards: [] }],
        ['staking', 'payee', [validator], 'Stash'],
        ['staking', 'erasStakersClipped', [era, validator], null],
        ['staking', 'erasStakersOverview', [era, validator], { total: '320', own: '128', nominatorCount: nominators.length, pageCount: 1 }],
        ['staking', 'erasStakersPaged', [era, validator, 0], { pageTotal: '192', others: nominators.map(nominator => ({ who: nominator.toString(), value: '12' })) }],
        ['staking', 'erasValidatorPrefs', [era, validator], { commission: 0, blocked: false }],
        ['staking', 'claimedRewards', [era, validator], []],
      );
    }
    const fixture = await clone(block, 'three fresh synthetic validator payout pages', rows);
    const payout = index => index % 2 === 0 ? meta.tx.staking.payoutStakers(validators[index], era)
      : meta.tx.staking.payoutStakersByPage(validators[index], era, 0);
    const payoutWeights = [];
    for (let index = 0; index < validators.length; index++) {
      const bare = new GenericExtrinsic(registry, payout(index));
      const response = await fixture.call('TransactionPaymentApi_query_info',
        [bare.toHex(), encode(registry.createType('u32', bare.encodedLength))]);
      const info = registry.createType('RuntimeDispatchInfo', response.result);
      assert.equal(info.partialFee.toBigInt(), 0n, 'Bare quote must remain free without waiving signed fees');
      payoutWeights.push(info.weight);
    }
    evidence.payoutDeclaredWeights = payoutWeights.map(weight => weight.toJSON());
    progress('Exact Wasm: payout-only batches use the 1/100 fee and retain execution weights');
    for (const kind of ['batch', 'batchAll', 'forceBatch']) {
      for (const count of [1, 2, 3]) {
        const call = meta.tx.utility[kind](Array.from({ length: count }, (_, index) => payout(index)));
        const admissionQuote = await quote(fixture, call, count, true);
        for (const component of ['refTime', 'proofSize']) {
          const innerWeight = payoutWeights.slice(0, count).reduce((sum, weight) => sum + weight[component].toBigInt(), 0n);
          assert(admissionQuote.info.weight[component].toBigInt() >= innerWeight,
            'Batch fee discount must not remove the declared weight of any inner payout');
        }
        if (!fitsExtrinsic(admissionQuote.info.weight)) {
          evidence.cases.push({ label: 'payoutFee.' + kind + '.' + count + '.overweightQuote',
            payoutCount: count, passed: true, quoteOnly: true, admissible: false,
            declaredWeight: admissionQuote.info.weight.toJSON(),
            quotedFee: admissionQuote.info.partialFee.toString(),
            ordinaryQuotedFeeCounterfactual: admissionQuote.ordinary.toString(), queryDetailsMatch: true });
          await reject(fixture, 'payoutFee.' + kind + '.' + count + '.overweight', call, 'ExhaustsResources');
          continue;
        }
        const result = await run(fixture, 'payoutFee.' + kind + '.' + count + '.success', call, count, true);
        assert.equal(result.result.rewardEvents, count * (nominators.length + 1));
        assert(result.result.refunded, 'Sixteen-nominator exposure should refund unused declared payout weight');
        assert(BigInt(result.result.xorCharged) > scaled(BigInt(count) * minimum), 'Success must exercise the percentage tariff above its minimum');
        for (const validator of validators.slice(0, count)) {
          assert.deepEqual((await rawQuery(result.block, 'staking', 'claimedRewards', [era, validator])).toJSON(), [0]);
          assert((await rawQuery(result.block, 'tokens', 'accounts', [validator, val])).free.toBigInt() > 0n);
        }
        if (count === 1) {
          const replay = await run(result.block, 'payoutFee.' + kind + '.replay', call, 1, true,
            kind !== 'batchAll', kind === 'batchAll' ? 'AlreadyClaimed' : null);
          assert.equal(replay.result.rewardEvents, 0);
          assert(BigInt(replay.result.xorCharged) >= scaled(minimum));
        }
      }
    }
    for (const kind of ['batch', 'batchAll', 'forceBatch']) {
      const invalid = meta.tx.staking.payoutStakersByPage(validators[0], era + 2, 0);
      const result = await run(fixture, 'payoutFee.' + kind + '.invalidEra', meta.tx.utility[kind]([invalid]),
        1, true, kind !== 'batchAll', kind === 'batchAll' ? 'InvalidEraToReward' : null);
      assert.equal(result.result.rewardEvents, 0);
    }
    const atomic = await run(fixture, 'payoutFee.batchAll.atomicRollback', meta.tx.utility.batchAll([payout(0), payout(0)]),
      2, true, false, 'AlreadyClaimed');
    assert.equal(atomic.result.rewardEvents, 0);
    assert.deepEqual((await rawQuery(atomic.block, 'staking', 'claimedRewards', [era, validators[0]])).toJSON(), []);
    const invalidFirst = meta.tx.staking.payoutStakersByPage(validators[0], era + 2, 0);
    const interrupted = await run(fixture, 'payoutFee.batch.interruptedSuffix', meta.tx.utility.batch([invalidFirst, payout(1)]), 2, true);
    assert.equal(interrupted.result.rewardEvents, 0);
    assert.deepEqual((await rawQuery(interrupted.block, 'staking', 'claimedRewards', [era, validators[1]])).toJSON(), []);
    assert(BigInt(interrupted.result.xorCharged) >= scaled(2n * minimum));
    const continued = await run(fixture, 'payoutFee.forceBatch.continuesAfterFailure', meta.tx.utility.forceBatch([invalidFirst, payout(1)]), 2, true);
    assert.equal(continued.result.rewardEvents, nominators.length + 1);
    progress('Exact Wasm: empty, mixed, nested and xorless payout wrappers retain ordinary fees');
    for (const kind of ['batch', 'batchAll', 'forceBatch']) {
      await run(fixture, 'payoutFee.' + kind + '.empty.fullFee', meta.tx.utility[kind]([]), 0, false);
      await run(fixture, 'payoutFee.' + kind + '.mixed.fullFee', meta.tx.utility[kind]([payout(0), meta.tx.system.remark('0x01')]), 1, false);
      await run(fixture, 'payoutFee.' + kind + '.nested.fullFee', meta.tx.utility[kind]([meta.tx.utility.batch([payout(0)])]), 1, false);
    }
    // Mainnet xorless support may intentionally reject dispatch; its quotation must still be ordinary.
    const xorlessCall = meta.tx.xorFee.xorlessCall(meta.tx.utility.batch([payout(0)]), null);
    const xorless = await quote(fixture, xorlessCall, 1, false);
    evidence.cases.push({ label: 'payoutFee.xorless.fullFee', passed: true, discounted: false,
      quoteOnly: true, quotedFee: xorless.info.partialFee.toString(), queryDetailsMatch: true });
    const unfundedAccount = (await rawQuery(fixture, 'system', 'account', [payer])).toJSON();
    unfundedAccount.data.free = '0';
    const unfunded = await clone(fixture, 'zero-XOR fee payer', [
      ['system', 'account', [payer], unfundedAccount],
      ['balances', 'totalIssuance', [], ((await rawQuery(fixture, 'balances', 'totalIssuance')).toBigInt() - 10_000n * unit).toString()],
    ]);
    await reject(unfunded, 'payoutFee.batch.unfunded', meta.tx.utility.batch([payout(0)]), 'Payment');
    await reject(fixture, 'payoutFee.batch.overweight', meta.tx.utility.batch(Array.from({ length: 10 }, () => payout(0))), 'ExhaustsResources');
    const adjusted = await clone(fixture, 'non-unit transaction-payment congestion multiplier', [
      ['transactionPayment', 'nextFeeMultiplier', [], (3n * unit / 2n).toString()],
    ]);
    await run(adjusted, 'payoutFee.batch.nextFeeMultiplier', meta.tx.utility.batch([payout(0)]), 1, true);
    evidence.safeguards = { nonemptyFlatPayoutsOnly: true, mixedNestedAndXorlessNotDiscounted: true,
      perSubmittedPayoutFloorRetained: true, failuresAndReplaysRemainPaid: true,
      interruptedSuffixRetainsFloor: true, atomicRollbackPreserved: true,
      zeroXorRejectedBeforeExecution: true, overweightRejectedBeforeExecution: true,
      declaredWeightRetainedAndActualBounded: true, congestionMultiplierIncluded: true,
      queryDetailsAndActualFeeEventsMatch: true };
    evidence.passed = true;
    return true;
  } finally {
    report.signer = previousSigner; save();
  }
}
module.exports = { payoutBatchFixture };
