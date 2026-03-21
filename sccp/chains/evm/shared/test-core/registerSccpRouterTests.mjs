import { expect } from 'chai';
import { network } from 'hardhat';

const routerTestConfig = globalThis.__SCCP_ROUTER_TEST_CONFIG;
if (!routerTestConfig) {
  throw new Error('__SCCP_ROUTER_TEST_CONFIG must be defined before loading router tests');
}

async function expectCustomError(promise, contract, name) {
  try {
    await promise;
    throw new Error('expected revert');
  } catch (e) {
    const data = e?.data ?? e?.error?.data ?? e?.info?.error?.data;
    if (!data) {
      throw e;
    }
    const decoded = contract.interface.parseError(data);
    expect(decoded?.name).to.equal(name);
  }
}

function text32(s, ethers) {
  return ethers.encodeBytes32String(s);
}

function malformedLabelWithTrailingNonZero() {
  return `0x410042${'00'.repeat(29)}`; // "A\0B..." violates canonical zero-padding.
}

describe('SCCP router (roleless, proof-driven token lifecycle)', function () {
  it('rejects invalid constructor inputs', async function () {
    const { ethers } = await network.connect();
    const Router = await ethers.getContractFactory('SccpRouter');
    const contractLike = { interface: Router.interface };

    await expectCustomError(Router.deploy(5, ethers.ZeroAddress), contractLike, 'ZeroAddress');
    await expectCustomError(
      Router.deploy(999, ethers.ZeroAddress),
      contractLike,
      'ZeroAddress',
    );

    const TrueVerifier = await ethers.getContractFactory('AlwaysTrueVerifier');
    const verifier = await TrueVerifier.deploy();
    await verifier.waitForDeployment();
    await expectCustomError(
      Router.deploy(999, await verifier.getAddress()),
      contractLike,
      'DomainUnsupported',
    );
  });

  it('adds tokens only from valid proofs and enforces governance replay protection', async function () {
    const { ethers } = await network.connect();
    const [user] = await ethers.getSigners();

    const LOCAL_DOMAIN = routerTestConfig.localDomain;
    const soraAssetId = `0x${'11'.repeat(32)}`;

    const TrueVerifier = await ethers.getContractFactory('AlwaysTrueVerifier');
    const verifier = await TrueVerifier.deploy();
    await verifier.waitForDeployment();

    const Router = await ethers.getContractFactory('SccpRouter');
    const router = await Router.deploy(LOCAL_DOMAIN, await verifier.getAddress());
    await router.waitForDeployment();

    const CodecTest = await ethers.getContractFactory('SccpCodecTest');
    const codec = await CodecTest.deploy();
    await codec.waitForDeployment();

    const addPayload = await codec.encodeTokenAddPayloadV1(
      LOCAL_DOMAIN,
      1,
      soraAssetId,
      18,
      text32('SCCP Wrapped', ethers),
      text32('wSORA', ethers),
    );
    const addMessageId = await codec.tokenAddMessageId(addPayload);

    await (await router.addTokenFromProof(addPayload, '0x')).wait();

    const tokenAddr = await router.tokenBySoraAssetId(soraAssetId);
    expect(tokenAddr).to.not.equal(ethers.ZeroAddress);
    expect(await router.processedGovernanceMessage(addMessageId)).to.equal(true);
    expect(await router.tokenStateBySoraAssetId(soraAssetId)).to.equal(1n);

    const token = await ethers.getContractAt('SccpToken', tokenAddr);
    expect(await token.name()).to.equal('SCCP Wrapped');
    expect(await token.symbol()).to.equal('wSORA');
    expect(await token.decimals()).to.equal(18n);
    expect(await token.balanceOf(user.address)).to.equal(0n);

    await expectCustomError(
      router.addTokenFromProof(addPayload, '0x'),
      router,
      'GovernanceActionAlreadyProcessed',
    );
  });

  it('rejects token-add proof failures and malformed governance payloads', async function () {
    const { ethers } = await network.connect();

    const LOCAL_DOMAIN = routerTestConfig.localDomain;
    const OTHER_EVM_DOMAIN = routerTestConfig.otherEvmDomain;
    const soraAssetId = `0x${'22'.repeat(32)}`;

    const FalseVerifier = await ethers.getContractFactory('AlwaysFalseVerifier');
    const falseVerifier = await FalseVerifier.deploy();
    await falseVerifier.waitForDeployment();

    const Router = await ethers.getContractFactory('SccpRouter');
    const router = await Router.deploy(LOCAL_DOMAIN, await falseVerifier.getAddress());
    await router.waitForDeployment();

    const CodecTest = await ethers.getContractFactory('SccpCodecTest');
    const codec = await CodecTest.deploy();
    await codec.waitForDeployment();

    const payload = await codec.encodeTokenAddPayloadV1(
      LOCAL_DOMAIN,
      2,
      soraAssetId,
      18,
      text32('Token', ethers),
      text32('TOK', ethers),
    );
    await expectCustomError(router.addTokenFromProof(payload, '0x'), router, 'ProofVerificationFailed');

    const TrueVerifier = await ethers.getContractFactory('AlwaysTrueVerifier');
    const verifier = await TrueVerifier.deploy();
    await verifier.waitForDeployment();

    const router2 = await Router.deploy(LOCAL_DOMAIN, await verifier.getAddress());
    await router2.waitForDeployment();

    const wrongDomainPayload = await codec.encodeTokenAddPayloadV1(
      OTHER_EVM_DOMAIN,
      3,
      soraAssetId,
      18,
      text32('Token', ethers),
      text32('TOK', ethers),
    );
    await expectCustomError(
      router2.addTokenFromProof(wrongDomainPayload, '0x'),
      router2,
      'DomainUnsupported',
    );

    const badNamePayload = await codec.encodeTokenAddPayloadV1(
      LOCAL_DOMAIN,
      4,
      soraAssetId,
      18,
      malformedLabelWithTrailingNonZero(),
      text32('TOK', ethers),
    );
    await expectCustomError(
      router2.addTokenFromProof(badNamePayload, '0x'),
      router2,
      'TokenMetadataInvalid',
    );
  });

  it('supports pause/resume via proofs and blocks burn/mint while paused', async function () {
    const { ethers } = await network.connect();
    const [user] = await ethers.getSigners();

    const DOMAIN_SORA = 0;
    const DOMAIN_SOL = 3;
    const LOCAL_DOMAIN = routerTestConfig.localDomain;
    const soraAssetId = `0x${'33'.repeat(32)}`;

    const TrueVerifier = await ethers.getContractFactory('AlwaysTrueVerifier');
    const verifier = await TrueVerifier.deploy();
    await verifier.waitForDeployment();

    const Router = await ethers.getContractFactory('SccpRouter');
    const router = await Router.deploy(LOCAL_DOMAIN, await verifier.getAddress());
    await router.waitForDeployment();

    const CodecTest = await ethers.getContractFactory('SccpCodecTest');
    const codec = await CodecTest.deploy();
    await codec.waitForDeployment();

    const addPayload = await codec.encodeTokenAddPayloadV1(
      LOCAL_DOMAIN,
      1,
      soraAssetId,
      18,
      text32('SCCP Wrapped', ethers),
      text32('wSORA', ethers),
    );
    await (await router.addTokenFromProof(addPayload, '0x')).wait();

    const tokenAddr = await router.tokenBySoraAssetId(soraAssetId);
    const token = await ethers.getContractAt('SccpToken', tokenAddr);

    const inboundPayload = await codec.encodeBurnPayloadV1(
      DOMAIN_SORA,
      LOCAL_DOMAIN,
      10,
      soraAssetId,
      5,
      ethers.zeroPadValue(user.address, 32),
    );
    await (await router.mintFromProof(DOMAIN_SORA, inboundPayload, '0x')).wait();
    expect(await token.balanceOf(user.address)).to.equal(5n);

    const pausePayload = await codec.encodeTokenPausePayloadV1(LOCAL_DOMAIN, 2, soraAssetId);
    const pauseMessageId = await codec.tokenPauseMessageId(pausePayload);
    await (await router.pauseTokenFromProof(pausePayload, '0x')).wait();
    expect(await router.processedGovernanceMessage(pauseMessageId)).to.equal(true);
    expect(await router.tokenStateBySoraAssetId(soraAssetId)).to.equal(2n);

    await (await token.connect(user).approve(await router.getAddress(), 1n)).wait();
    await expectCustomError(
      router.connect(user).burnToDomain(soraAssetId, 1n, DOMAIN_SOL, `0x${'44'.repeat(32)}`),
      router,
      'TokenNotActive',
    );

    const pausedInboundPayload = await codec.encodeBurnPayloadV1(
      DOMAIN_SORA,
      LOCAL_DOMAIN,
      11,
      soraAssetId,
      1,
      ethers.zeroPadValue(user.address, 32),
    );
    await expectCustomError(
      router.mintFromProof(DOMAIN_SORA, pausedInboundPayload, '0x'),
      router,
      'TokenNotActive',
    );

    await expectCustomError(
      router.pauseTokenFromProof(pausePayload, '0x'),
      router,
      'GovernanceActionAlreadyProcessed',
    );

    const resumePayload = await codec.encodeTokenResumePayloadV1(LOCAL_DOMAIN, 3, soraAssetId);
    const resumeMessageId = await codec.tokenResumeMessageId(resumePayload);
    await (await router.resumeTokenFromProof(resumePayload, '0x')).wait();
    expect(await router.processedGovernanceMessage(resumeMessageId)).to.equal(true);
    expect(await router.tokenStateBySoraAssetId(soraAssetId)).to.equal(1n);

    await (await router.connect(user).burnToDomain(soraAssetId, 1n, DOMAIN_SOL, `0x${'55'.repeat(32)}`)).wait();
    expect(await token.balanceOf(user.address)).to.equal(4n);
  });

  it('emits a canonical outbound burn proof target', async function () {
    const { ethers } = await network.connect();
    const [user] = await ethers.getSigners();

    const DOMAIN_SORA = 0;
    const LOCAL_DOMAIN = routerTestConfig.localDomain;
    const soraAssetId = `0x${'88'.repeat(32)}`;
    const outboundRecipient = ethers.encodeBytes32String('sora-recipient');

    const TrueVerifier = await ethers.getContractFactory('AlwaysTrueVerifier');
    const verifier = await TrueVerifier.deploy();
    await verifier.waitForDeployment();

    const Router = await ethers.getContractFactory('SccpRouter');
    const router = await Router.deploy(LOCAL_DOMAIN, await verifier.getAddress());
    await router.waitForDeployment();

    const CodecTest = await ethers.getContractFactory('SccpCodecTest');
    const codec = await CodecTest.deploy();
    await codec.waitForDeployment();

    const addPayload = await codec.encodeTokenAddPayloadV1(
      LOCAL_DOMAIN,
      1,
      soraAssetId,
      18,
      text32('SCCP Wrapped', ethers),
      text32('wSORA', ethers),
    );
    await (await router.addTokenFromProof(addPayload, '0x')).wait();

    const inboundPayload = await codec.encodeBurnPayloadV1(
      DOMAIN_SORA,
      LOCAL_DOMAIN,
      2,
      soraAssetId,
      9,
      ethers.zeroPadValue(user.address, 32),
    );
    await (await router.mintFromProof(DOMAIN_SORA, inboundPayload, '0x')).wait();

    const tokenAddr = await router.tokenBySoraAssetId(soraAssetId);
    const token = await ethers.getContractAt('SccpToken', tokenAddr);
    await (await token.connect(user).approve(await router.getAddress(), 3n)).wait();

    const burnTx = await router.connect(user).burnToDomain(soraAssetId, 3n, DOMAIN_SORA, outboundRecipient);
    const burnReceipt = await burnTx.wait();
    const eventFragment = router.interface.getEvent('SccpBurned');
    const routerAddr = await router.getAddress();
    expect(await router.BURN_EVENT_TOPIC0()).to.equal(eventFragment.topicHash);

    const burnLog = burnReceipt.logs.find(
      (log) => log.address === routerAddr && log.topics[0] === eventFragment.topicHash,
    );
    expect(burnLog).to.not.equal(undefined);

    const decoded = router.interface.parseLog(burnLog);
    expect(decoded?.name).to.equal('SccpBurned');
    expect(decoded.args.sender).to.equal(user.address);
    expect(decoded.args.soraAssetId).to.equal(soraAssetId);
    expect(decoded.args.amount).to.equal(3n);
    expect(decoded.args.destDomain).to.equal(0n);
    expect(decoded.args.recipient).to.equal(outboundRecipient);
    expect(decoded.args.nonce).to.equal(1n);

    const payload = decoded.args.payload;
    const messageId = decoded.args.messageId;
    expect(await codec.burnMessageId(payload)).to.equal(messageId);
    expect(await router.burnPayload(messageId)).to.equal(payload);

    const burnPayload = await codec.decodeBurnPayloadV1(payload);
    expect(burnPayload.version).to.equal(1n);
    expect(burnPayload.sourceDomain).to.equal(BigInt(LOCAL_DOMAIN));
    expect(burnPayload.destDomain).to.equal(0n);
    expect(burnPayload.nonce).to.equal(1n);
    expect(burnPayload.soraAssetId).to.equal(soraAssetId);
    expect(burnPayload.amount).to.equal(3n);
    expect(burnPayload.recipient).to.equal(outboundRecipient);
  });

  it('keeps burn/mint replay and recipient canonical protections', async function () {
    const { ethers } = await network.connect();
    const [user] = await ethers.getSigners();

    const DOMAIN_SORA = 0;
    const LOCAL_DOMAIN = routerTestConfig.localDomain;
    const OTHER_EVM_DOMAIN = routerTestConfig.otherEvmDomain;
    const soraAssetId = `0x${'44'.repeat(32)}`;

    const TrueVerifier = await ethers.getContractFactory('AlwaysTrueVerifier');
    const verifier = await TrueVerifier.deploy();
    await verifier.waitForDeployment();

    const Router = await ethers.getContractFactory('SccpRouter');
    const router = await Router.deploy(LOCAL_DOMAIN, await verifier.getAddress());
    await router.waitForDeployment();

    const CodecTest = await ethers.getContractFactory('SccpCodecTest');
    const codec = await CodecTest.deploy();
    await codec.waitForDeployment();

    const addPayload = await codec.encodeTokenAddPayloadV1(
      LOCAL_DOMAIN,
      1,
      soraAssetId,
      18,
      text32('SCCP Wrapped', ethers),
      text32('wSORA', ethers),
    );
    await (await router.addTokenFromProof(addPayload, '0x')).wait();

    const payload = await codec.encodeBurnPayloadV1(
      DOMAIN_SORA,
      LOCAL_DOMAIN,
      5,
      soraAssetId,
      1,
      ethers.zeroPadValue(user.address, 32),
    );

    await (await router.mintFromProof(DOMAIN_SORA, payload, '0x')).wait();
    await expectCustomError(
      router.mintFromProof(DOMAIN_SORA, payload, '0x'),
      router,
      'InboundAlreadyProcessed',
    );

    const badRecipientPayload = await codec.encodeBurnPayloadV1(
      DOMAIN_SORA,
      LOCAL_DOMAIN,
      6,
      soraAssetId,
      1,
      `0x${'ff'.repeat(32)}`,
    );
    await expectCustomError(
      router.mintFromProof(DOMAIN_SORA, badRecipientPayload, '0x'),
      router,
      'RecipientNotCanonical',
    );

    const tokenAddr = await router.tokenBySoraAssetId(soraAssetId);
    const token = await ethers.getContractAt('SccpToken', tokenAddr);
    await (await token.connect(user).approve(await router.getAddress(), 1n)).wait();

    await expectCustomError(
      router
        .connect(user)
        .burnToDomain(soraAssetId, 1n, OTHER_EVM_DOMAIN, `0x${'ff'.repeat(32)}`),
      router,
      'RecipientNotCanonical',
    );
  });

  it('does not consume governance replay slot when duplicate asset registration is rejected', async function () {
    const { ethers } = await network.connect();

    const LOCAL_DOMAIN = routerTestConfig.localDomain;
    const soraAssetId = `0x${'77'.repeat(32)}`;

    const TrueVerifier = await ethers.getContractFactory('AlwaysTrueVerifier');
    const verifier = await TrueVerifier.deploy();
    await verifier.waitForDeployment();

    const Router = await ethers.getContractFactory('SccpRouter');
    const router = await Router.deploy(LOCAL_DOMAIN, await verifier.getAddress());
    await router.waitForDeployment();

    const CodecTest = await ethers.getContractFactory('SccpCodecTest');
    const codec = await CodecTest.deploy();
    await codec.waitForDeployment();

    const addPayloadV1 = await codec.encodeTokenAddPayloadV1(
      LOCAL_DOMAIN,
      1,
      soraAssetId,
      18,
      text32('SCCP Wrapped', ethers),
      text32('wSORA', ethers),
    );
    await (await router.addTokenFromProof(addPayloadV1, '0x')).wait();
    const tokenAddr = await router.tokenBySoraAssetId(soraAssetId);

    const addPayloadV2 = await codec.encodeTokenAddPayloadV1(
      LOCAL_DOMAIN,
      2,
      soraAssetId,
      9,
      text32('SCCP Wrapped 2', ethers),
      text32('wSORA2', ethers),
    );
    const addMessageIdV2 = await codec.tokenAddMessageId(addPayloadV2);
    expect(await router.processedGovernanceMessage(addMessageIdV2)).to.equal(false);

    await expectCustomError(router.addTokenFromProof(addPayloadV2, '0x'), router, 'TokenAlreadyRegistered');
    expect(await router.tokenBySoraAssetId(soraAssetId)).to.equal(tokenAddr);
    expect(await router.processedGovernanceMessage(addMessageIdV2)).to.equal(false);
  });
});
