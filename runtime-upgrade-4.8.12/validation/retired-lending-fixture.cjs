'use strict';
// Local exact-Wasm fixtures for retirement. Every synthetic write is disclosed.
const assert = require('node:assert/strict');
const { u8aToHex } = require('@polkadot/util');
const { xxhashAsU8a } = require('@polkadot/util-crypto');

async function retiredLendingFixture(block, context) {
  const { chain, Block, report, rawQuery, storageKey, signedCheck, unsignedCheck,
    progress, save, syntheticFixtures } = context;
  assert(syntheticFixtures, 'Retired lending evidence requires --synthetic-fixtures');
  const meta = await block.meta;
  const registry = meta.registry;
  const encode = value => u8aToHex(value.toU8a());
  const unit = 10n ** 18n;
  const amount = value => (BigInt(value) * unit).toString();
  const pswap = '0x0200050000000000000000000000000000000000000000000000000000000000';
  const val = '0x0200040000000000000000000000000000000000000000000000000000000000';
  const kusd = '0x02000c0000000000000000000000000000000000000000000000000000000000';
  const apolloAsset = '0x00efe45135018136733be626b380a87ae663ccf6784a25fe9d9d2be64acecb9d';
  const who = registry.createType('AccountId', '0x' + '79'.repeat(32));
  const apollo = registry.createType('AccountId', Buffer.concat([
    Buffer.from('modlapollolb'), Buffer.alloc(20),
  ]));
  const cdpId = (1n << 120n).toString();
  const pair = { collateralAssetId: pswap, stablecoinAssetId: kusd };
  const previousSigner = report.signer;
  const evidence = report.retiredLending = {
    passed: false, synthetic: true, constantChecks: {}, blockedCalls: [],
    kensetsu: {}, apollo: {},
    limitations: [
      'Synthetic CDP, pool, position, account and issuance overrides are local fixtures, not an audit of every live borrower or lender.',
      'Each call resets only synthetic block weight/size accounting; this does not claim all calls fit in one real block.',
      'Transaction signatures use the rehearsal mock host. Native Executive tests cover real transaction signatures.',
    ],
  };
  const valueType = (pallet, name) => {
    const type = meta.query[pallet][name].meta.type;
    return registry.createLookupType(type.isMap ? type.asMap.value : type.asPlain);
  };
  const defaultValue = (pallet, name) => registry.createType(valueType(pallet, name)).toJSON();
  const override = async (parent, label, rows) => {
    const writes = [];
    for (const [pallet, name, params, json] of rows) {
      const key = storageKey(meta, pallet, name, params);
      const encoded = json === null ? null : encode(registry.createType(key.outputType, json));
      const before = await parent.get(key.toHex());
      writes.push([key.toHex(), encoded]);
      report.fixtureOverrides.push({ kind: 'retiredLending', fixtureOnly: true, label,
        pallet, name, key: key.toHex(), value: encoded, parentStateBefore: before ?? null });
    }
    const branch = new Block(chain, parent.number, parent.hash, parent, {
      header: await parent.header, extrinsics: [], storage: parent.storage,
    });
    branch.pushStorageLayer().setAll(writes);
    return branch;
  };
  const execute = async (parent, label, call, success = true, error = null) => {
    const clean = await override(parent, label + ' isolated transaction accounting', [
      ['system', 'blockWeight', [], null], ['system', 'blockSize', [], null],
    ]);
    return signedCheck(clean, label, call, success, false, error);
  };
  const fundToken = async (parent, label, owner, asset, free, reserved = 0n) => {
    const old = await rawQuery(parent, 'tokens', 'accounts', [owner, asset]);
    const token = old.toJSON(); token.free = free.toString(); token.reserved = reserved.toString();
    const issuance = (await rawQuery(parent, 'tokens', 'totalIssuance', [asset])).toBigInt();
    const nextIssuance = issuance + free + reserved - old.free.toBigInt() - old.reserved.toBigInt();
    assert(nextIssuance >= 0n, label + ' synthetic token issuance must remain nonnegative');
    const rows = [
      ['tokens', 'accounts', [owner, asset], token],
      ['tokens', 'totalIssuance', [asset], nextIssuance.toString()],
    ];
    if (!(await parent.get(storageKey(meta, 'tokens', 'accounts', [owner, asset]).toHex()))) {
      const system = (await rawQuery(parent, 'system', 'account', [owner])).toJSON();
      system.providers = Number(system.providers) + 1;
      rows.push(['system', 'account', [owner], system]);
    }
    return override(parent, label, rows);
  };
  const free = async (parent, owner, asset) =>
    (await rawQuery(parent, 'tokens', 'accounts', [owner, asset])).free.toBigInt();
  const snapshotRows = async (parent, rows) => Promise.all(rows.map(async ([pallet, name, params]) =>
    encode(await rawQuery(parent, pallet, name, params))));
  const pool = values => ({ ...defaultValue('apolloPlatform', 'poolData'), ...values });
  const position = (principal, interest = 0, rewards = 0) => ({
    collateralAmount: amount(200), borrowingAmount: amount(principal),
    borrowingInterest: amount(interest), lastBorrowingBlock: block.number,
    borrowingRewards: amount(rewards),
  });
  const lending = (principal, rewards = 0) => ({
    lendingAmount: amount(principal), lendingInterest: amount(rewards), lastLendingBlock: block.number,
  });
  const loan = async parent => (await rawQuery(parent, 'apolloPlatform', 'userBorrowingInfo', [pswap, who])).unwrap();
  const onlyPosition = async parent => {
    const positions = await loan(parent);
    assert.equal(positions.size, 1);
    return [...positions.values()][0];
  };
  const apolloFixture = async (parent, label, interest = 0, rewards = 0, lenderRewards = null) => {
    let next = await override(parent, label, [
      ['apolloPlatform', 'poolData', [pswap], pool({ totalBorrowed: amount(100) })],
      ['apolloPlatform', 'poolData', [val], pool({ totalCollateral: amount(200), totalLiquidity: amount(lenderRewards === null ? 0 : 100) })],
      ['apolloPlatform', 'userBorrowingInfo', [pswap, who], new Map([[meta.tx.apolloPlatform.getRewards(val, false).args[0], position(100, interest, rewards)]])],
      ['apolloPlatform', 'userTotalCollateral', [who, val], amount(200)],
      ['apolloPlatform', 'deferredProtocolInterest', [pswap], '0'],
      ['apolloPlatform', 'userLendingInfo', [val, who], lenderRewards === null ? null : lending(100, lenderRewards)],
    ]);
    next = await fundToken(next, label + ' borrower principal and interest', who, pswap, (100n + BigInt(interest)) * unit);
    next = await fundToken(next, label + ' collateral and lender principal escrow', apollo, val,
      (lenderRewards === null ? 200n : 300n) * unit);
    next = await fundToken(next, label + ' isolated repayment escrow', apollo, pswap, 0n);
    next = await fundToken(next, label + ' unfunded APOLLO reward escrow', apollo, apolloAsset, 0n);
    return next;
  };
  try {
    for (const pallet of ['kensetsu', 'apolloPlatform']) {
      assert(meta.consts[pallet].repaymentOnly.isTrue, pallet + ' must expose RepaymentOnly=true');
      evidence.constantChecks[pallet] = true;
    }
    report.signer = { address: who.toString(), synthetic: true, purpose: 'Retired lending exact-Wasm fixture' };
    const account = (await rawQuery(block, 'system', 'account', [who])).toJSON();
    assert.equal(BigInt(account.data.free), 0n, 'Synthetic retired-lending payer must not collide with a funded public account');
    account.nonce = 0; account.providers = Number(account.providers) + 1;
    account.data.free = (10n ** 30n).toString();
    const funded = await override(block, 'funded synthetic exit owner', [
      ['system', 'account', [who], account],
      ['balances', 'totalIssuance', [], ((await rawQuery(block, 'balances', 'totalIssuance')).toBigInt() + 10n ** 30n).toString()],
    ]);
    const blocked = [
      ['kensetsu.createCdp', meta.tx.kensetsu.createCdp(pswap, amount(200), kusd, 0, amount(100), 'Type2')],
      ['kensetsu.depositCollateral', meta.tx.kensetsu.depositCollateral(cdpId, amount(1))],
      ['kensetsu.borrow', meta.tx.kensetsu.borrow(cdpId, 0, amount(1))],
      ['kensetsu.accrue', meta.tx.kensetsu.accrue(cdpId)],
      ['kensetsu.liquidate', meta.tx.kensetsu.liquidate(cdpId)],
      ['kensetsu.donate', meta.tx.kensetsu.donate(kusd, amount(1))],
      ['apolloPlatform.lend', meta.tx.apolloPlatform.lend(val, amount(1))],
      ['apolloPlatform.borrow', meta.tx.apolloPlatform.borrow(val, pswap, amount(1), amount(1))],
      ['apolloPlatform.addCollateral', meta.tx.apolloPlatform.addCollateral(val, amount(1), pswap)],
      ['apolloPlatform.liquidate', meta.tx.apolloPlatform.liquidate(who, pswap)],
    ];
    progress('Exact Wasm: retired lending activities fail with paid RepaymentOnly through direct and batch dispatch');
    for (const [label, call] of blocked) {
      for (const [kind, wrapped] of [['direct', call], ['batchAll', meta.tx.utility.batchAll([call])]]) {
        await execute(funded, label + ' retirement ' + kind, wrapped, false, 'RepaymentOnly');
        evidence.blockedCalls.push(label + ':' + kind);
      }
    }
    for (const [label, call] of blocked.filter(([label]) => /\.accrue$|\.liquidate$/.test(label))) {
      await unsignedCheck(funded, label + ' retirement bare maintenance', call);
    }

    progress('Exact Wasm: retired Kensetsu partial repayment and closure return existing collateral');
    const technicalType = registry.createLookupType(meta.query.technical.techAccounts.meta.type.asMap.value);
    const technical = registry.createType(technicalType, {
      Generic: [u8aToHex(Buffer.from('kensetsu')), u8aToHex(Buffer.from('depository'))],
    });
    const depository = registry.createType('AccountId', Buffer.concat([
      Buffer.from([84, 115, 79, 144, 249, 113, 160, 44, 96, 155, 45, 104, 78, 97, 181, 87]),
      Buffer.from(xxhashAsU8a(technical.toU8a(), 128)),
    ]));
    assert((await rawQuery(block, 'technical', 'techAccounts', [depository])).unwrap().eq(technical));
    assert((await rawQuery(block, 'kensetsu', 'cdpDepository', [cdpId])).isNone);
    const collateral = defaultValue('kensetsu', 'collateralInfos');
    collateral.totalCollateral = amount(200); collateral.stablecoinSupply = amount(100);
    collateral.interestCoefficient = amount(1); collateral.lastFeeUpdateTime = 0;
    collateral.riskParameters.stabilityFeeRate = '0';
    let kensetsu = await override(funded, 'existing zero-rate collateralized debt', [
      ['kensetsu', 'cdpDepository', [cdpId], { owner: who.toString(), collateralAssetId: pswap,
        collateralAmount: amount(200), stablecoinAssetId: kusd, debt: amount(100), interestCoefficient: amount(1) }],
      ['kensetsu', 'cdpOwnerIndex', [who], [cdpId]],
      ['kensetsu', 'collateralInfos', [pair], collateral],
    ]);
    kensetsu = await fundToken(kensetsu, 'backed Kensetsu stablecoin repayment', who, kusd, 100n * unit);
    kensetsu = await fundToken(kensetsu, 'backed Kensetsu collateral escrow', depository, pswap, 200n * unit);
    const pswapBefore = await free(kensetsu, who, pswap);
    const kusdIssuance = (await rawQuery(kensetsu, 'tokens', 'totalIssuance', [kusd])).toBigInt();
    kensetsu = await execute(kensetsu, 'kensetsu.repayDebt retired partial repayment', meta.tx.kensetsu.repayDebt(cdpId, amount(40)));
    assert.equal((await rawQuery(kensetsu, 'kensetsu', 'cdpDepository', [cdpId])).unwrap().debt.toBigInt(), 60n * unit);
    assert.equal(await free(kensetsu, who, kusd), 60n * unit);
    assert.equal(await free(kensetsu, who, pswap), pswapBefore);
    assert.equal((await rawQuery(kensetsu, 'tokens', 'totalIssuance', [kusd])).toBigInt(), kusdIssuance - 40n * unit);
    kensetsu = await execute(kensetsu, 'kensetsu.closeCdp retired owner exit', meta.tx.kensetsu.closeCdp(cdpId));
    assert((await rawQuery(kensetsu, 'kensetsu', 'cdpDepository', [cdpId])).isNone);
    assert((await rawQuery(kensetsu, 'kensetsu', 'cdpOwnerIndex', [who])).isNone);
    assert.equal(await free(kensetsu, who, kusd), 0n);
    assert.equal(await free(kensetsu, who, pswap), pswapBefore + 200n * unit);
    assert.equal(await free(kensetsu, depository, pswap), 0n);
    assert.equal((await rawQuery(kensetsu, 'tokens', 'totalIssuance', [kusd])).toBigInt(), kusdIssuance - 100n * unit);
    const closedCollateral = (await rawQuery(kensetsu, 'kensetsu', 'collateralInfos', [pair])).unwrap();
    assert.equal(closedCollateral.totalCollateral.toBigInt(), 0n);
    assert.equal(closedCollateral.stablecoinSupply.toBigInt(), 0n);
    evidence.kensetsu = { paidPartialRepayment: true, paidClosure: true, collateralReturned: true,
      debtAndOwnerIndexRemoved: true, stablecoinBurnMatchesDebt: true, principal: amount(100), collateral: amount(200) };

    progress('Exact Wasm: existing Kensetsu interest continues accruing during partial repayment and closure');
    const treasuryTechnical = registry.createType(technicalType, {
      Generic: [u8aToHex(Buffer.from('kensetsu')), u8aToHex(Buffer.from('treasury'))],
    });
    const treasury = registry.createType('AccountId', Buffer.concat([
      Buffer.from([84, 115, 79, 144, 249, 113, 160, 44, 96, 155, 45, 104, 78, 97, 181, 87]),
      Buffer.from(xxhashAsU8a(treasuryTechnical.toU8a(), 128)),
    ]));
    assert((await rawQuery(block, 'technical', 'techAccounts', [treasury])).unwrap().eq(treasuryTechnical));
    const startSecond = (await rawQuery(funded, 'timestamp', 'now')).toBigInt() / 1000n;
    const interestCollateral = defaultValue('kensetsu', 'collateralInfos');
    interestCollateral.totalCollateral = amount(200); interestCollateral.stablecoinSupply = amount(100);
    interestCollateral.interestCoefficient = amount(1); interestCollateral.lastFeeUpdateTime = startSecond.toString();
    // Deliberately large synthetic rate makes one-second accrual exactly observable:
    // 100 grows to 110; repaying 40 leaves 70, which grows to 77 one second later.
    interestCollateral.riskParameters.stabilityFeeRate = (unit / 10n).toString();
    const stablecoin = (await rawQuery(funded, 'kensetsu', 'stablecoinInfos', [kusd])).unwrap().toJSON();
    stablecoin.badDebt = '0';
    let accrued = await override(funded, 'existing interest-bearing Kensetsu debt before time advance', [
      ['kensetsu', 'cdpDepository', [cdpId], { owner: who.toString(), collateralAssetId: pswap,
        collateralAmount: amount(200), stablecoinAssetId: kusd, debt: amount(100), interestCoefficient: amount(1) }],
      ['kensetsu', 'cdpOwnerIndex', [who], [cdpId]],
      ['kensetsu', 'collateralInfos', [pair], interestCollateral],
      ['kensetsu', 'stablecoinInfos', [kusd], stablecoin],
      ['timestamp', 'now', [], ((startSecond + 1n) * 1000n).toString()],
    ]);
    accrued = await fundToken(accrued, 'existing stablecoins funding principal and future accrued interest', who, kusd, 117n * unit);
    accrued = await fundToken(accrued, 'interest-bearing Kensetsu collateral escrow', depository, pswap, 200n * unit);
    const accruedCollateralBefore = await free(accrued, who, pswap);
    const treasuryBefore = await free(accrued, treasury, kusd);
    const accruedIssuanceBefore = (await rawQuery(accrued, 'tokens', 'totalIssuance', [kusd])).toBigInt();
    accrued = await execute(accrued, 'kensetsu.repayDebt retired partial with continuing interest', meta.tx.kensetsu.repayDebt(cdpId, amount(40)));
    const accruedRemaining = (await rawQuery(accrued, 'kensetsu', 'cdpDepository', [cdpId])).unwrap();
    assert.equal(accruedRemaining.debt.toBigInt(), 70n * unit);
    assert.equal(accruedRemaining.interestCoefficient.toBigInt(), 11n * unit / 10n);
    assert.equal((await rawQuery(accrued, 'kensetsu', 'collateralInfos', [pair])).unwrap().stablecoinSupply.toBigInt(), 70n * unit);
    assert.equal(await free(accrued, who, kusd), 77n * unit);
    assert.equal(await free(accrued, who, pswap), accruedCollateralBefore);
    assert.equal(await free(accrued, treasury, kusd), treasuryBefore + 10n * unit);
    assert.equal((await rawQuery(accrued, 'tokens', 'totalIssuance', [kusd])).toBigInt(), accruedIssuanceBefore + 10n * unit - 40n * unit);
    accrued = await override(accrued, 'advance one more second before Kensetsu closure', [
      ['timestamp', 'now', [], ((startSecond + 2n) * 1000n).toString()],
    ]);
    accrued = await execute(accrued, 'kensetsu.closeCdp retired exit with continuing interest', meta.tx.kensetsu.closeCdp(cdpId));
    assert((await rawQuery(accrued, 'kensetsu', 'cdpDepository', [cdpId])).isNone);
    assert((await rawQuery(accrued, 'kensetsu', 'cdpOwnerIndex', [who])).isNone);
    assert.equal(await free(accrued, who, kusd), 0n);
    assert.equal(await free(accrued, who, pswap), accruedCollateralBefore + 200n * unit);
    assert.equal(await free(accrued, depository, pswap), 0n);
    assert.equal(await free(accrued, treasury, kusd), treasuryBefore + 17n * unit);
    assert.equal((await rawQuery(accrued, 'tokens', 'totalIssuance', [kusd])).toBigInt(), accruedIssuanceBefore + 17n * unit - 117n * unit);
    const accruedClosed = (await rawQuery(accrued, 'kensetsu', 'collateralInfos', [pair])).unwrap();
    assert.equal(accruedClosed.stablecoinSupply.toBigInt(), 0n);
    assert.equal(accruedClosed.totalCollateral.toBigInt(), 0n);
    assert.equal((await rawQuery(accrued, 'kensetsu', 'stablecoinInfos', [kusd])).unwrap().badDebt.toBigInt(), 0n);
    Object.assign(evidence.kensetsu, { debtPolicy: 'existing-interest-unchanged',
      nonzeroRateInterestAccrues: true, existingTreasuryAccountingPreserved: true,
      syntheticStabilityRatePerSecond: (unit / 10n).toString(), elapsedSeconds: 2,
      accruedInterest: amount(17), treasuryMintFromAccrual: amount(17), totalStablecoinBurn: amount(117) });

    progress('Exact Wasm: Apollo repayment releases collateral and reserves paid interest without DEX conversion');
    let apolloExit = await apolloFixture(funded, 'existing Apollo loan with recorded interest', 10);
    const valBefore = await free(apolloExit, who, val);
    apolloExit = await execute(apolloExit, 'apolloPlatform.repay retired partial with interest', meta.tx.apolloPlatform.repay(val, pswap, amount(40)));
    assert.equal((await onlyPosition(apolloExit)).borrowingAmount.toBigInt(), 70n * unit);
    assert.equal((await onlyPosition(apolloExit)).borrowingInterest.toBigInt(), 0n);
    assert.equal(await free(apolloExit, who, val), valBefore);
    assert.equal((await rawQuery(apolloExit, 'apolloPlatform', 'deferredProtocolInterest', [pswap])).toBigInt(), 10n * unit);
    assert.equal((await rawQuery(apolloExit, 'tokens', 'accounts', [apollo, pswap])).reserved.toBigInt(), 10n * unit);
    apolloExit = await execute(apolloExit, 'apolloPlatform.repay retired full exit', meta.tx.apolloPlatform.repay(val, pswap, amount(70)));
    assert((await rawQuery(apolloExit, 'apolloPlatform', 'userBorrowingInfo', [pswap, who])).isNone);
    assert((await rawQuery(apolloExit, 'apolloPlatform', 'userTotalCollateral', [who, val])).isNone);
    assert.equal(await free(apolloExit, who, pswap), 0n);
    assert.equal(await free(apolloExit, who, val), valBefore + 200n * unit);
    const repaidPool = (await rawQuery(apolloExit, 'apolloPlatform', 'poolData', [pswap])).unwrap();
    assert.equal(repaidPool.totalBorrowed.toBigInt(), 0n);
    assert.equal(repaidPool.totalLiquidity.toBigInt(), 100n * unit);
    assert.equal((await rawQuery(apolloExit, 'apolloPlatform', 'poolData', [val])).unwrap().totalCollateral.toBigInt(), 0n);
    apolloExit = await override(apolloExit, 'last lender entitled to exactly the repaid principal', [
      ['apolloPlatform', 'userLendingInfo', [pswap, who], lending(100)],
    ]);
    apolloExit = await execute(apolloExit, 'apolloPlatform.withdraw exact last-lender liquidity', meta.tx.apolloPlatform.withdraw(pswap, amount(100)));
    assert((await rawQuery(apolloExit, 'apolloPlatform', 'userLendingInfo', [pswap, who])).isNone);
    assert.equal((await rawQuery(apolloExit, 'apolloPlatform', 'poolData', [pswap])).unwrap().totalLiquidity.toBigInt(), 0n);
    assert.equal(await free(apolloExit, who, pswap), 100n * unit);
    assert.equal(await free(apolloExit, apollo, pswap), 0n);
    assert.equal((await rawQuery(apolloExit, 'tokens', 'accounts', [apollo, pswap])).reserved.toBigInt(), 10n * unit);
    assert.equal((await rawQuery(apolloExit, 'apolloPlatform', 'deferredProtocolInterest', [pswap])).toBigInt(), 10n * unit);
    Object.assign(evidence.apollo, { paidPartialAndFullRepayment: true, collateralReturned: true,
      recordedInterestReserved: true, reserveExcludedFromLenderWithdrawal: true, exactLastLenderWithdrawal: true });

    progress('Exact Wasm: Apollo principal exits preserve earned rewards when the reward escrow is empty');
    let rewards = await apolloFixture(funded, 'existing earned borrower and lender rewards', 0, 7, 5);
    const rewardCollateralBefore = await free(rewards, who, val);
    rewards = await execute(rewards, 'apolloPlatform.repay exit with unfunded earned rewards', meta.tx.apolloPlatform.repay(val, pswap, amount(100)));
    rewards = await execute(rewards, 'apolloPlatform.withdraw exit with unfunded earned rewards', meta.tx.apolloPlatform.withdraw(val, amount(100)));
    const retainedBorrow = await onlyPosition(rewards);
    assert.equal(retainedBorrow.borrowingAmount.toBigInt(), 0n);
    assert.equal(retainedBorrow.collateralAmount.toBigInt(), 0n);
    assert.equal(retainedBorrow.borrowingRewards.toBigInt(), 7n * unit);
    const retainedLender = (await rawQuery(rewards, 'apolloPlatform', 'userLendingInfo', [val, who])).unwrap();
    assert.equal(retainedLender.lendingAmount.toBigInt(), 0n);
    assert.equal(retainedLender.lendingInterest.toBigInt(), 5n * unit);
    assert.equal(await free(rewards, who, val), rewardCollateralBefore + 300n * unit);
    const claims = [['apolloPlatform', 'userBorrowingInfo', [pswap, who]],
      ['apolloPlatform', 'userLendingInfo', [val, who]], ['apolloPlatform', 'poolData', [pswap]],
      ['apolloPlatform', 'poolData', [val]]];
    const claimsBefore = await snapshotRows(rewards, claims);
    rewards = await execute(rewards, 'apolloPlatform.getRewards empty escrow retains claim', meta.tx.apolloPlatform.getRewards(pswap, false), false, 'UnableToTransferRewards');
    assert.deepEqual(await snapshotRows(rewards, claims), claimsBefore);
    const earnedBefore = await free(rewards, who, apolloAsset);
    rewards = await fundToken(rewards, 'later local funding of previously earned APOLLO claims', apollo, apolloAsset, 12n * unit);
    rewards = await execute(rewards, 'apolloPlatform.getRewards retained borrowing claim', meta.tx.apolloPlatform.getRewards(pswap, false));
    rewards = await execute(rewards, 'apolloPlatform.getRewards retained lending claim', meta.tx.apolloPlatform.getRewards(val, true));
    assert.equal(await free(rewards, who, apolloAsset), earnedBefore + 12n * unit);
    assert.equal(await free(rewards, apollo, apolloAsset), 0n);
    assert((await rawQuery(rewards, 'apolloPlatform', 'userBorrowingInfo', [pswap, who])).isNone);
    assert((await rawQuery(rewards, 'apolloPlatform', 'userLendingInfo', [val, who])).isNone);
    Object.assign(evidence.apollo, { principalExitWithoutRewardFunding: true, earnedClaimsPreserved: true,
      failedClaimRollsBack: true, retainedClaimsRedeemAfterFunding: true });
    evidence.passed = true;
    return true;
  } finally {
    report.signer = previousSigner;
    save();
  }
}
module.exports = { retiredLendingFixture };
