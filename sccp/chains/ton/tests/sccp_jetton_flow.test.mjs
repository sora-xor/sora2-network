import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { Blockchain } from '@ton/sandbox';
import {
  beginCell,
  Cell,
  contractAddress,
  Dictionary,
  SendMode,
} from '@ton/core';
import { ethers } from 'ethers';

const repoRoot = resolve(import.meta.dirname, '..');

// Opcodes from `contracts/messages.tolk`.
const TOP_UP_TONS = 0xd372158c;
const SCCP_SET_VERIFIER = 0x0f95e281;
const SCCP_SET_INBOUND_PAUSED = 0x3bf64dc2;
const SCCP_SET_OUTBOUND_PAUSED = 0x91f4c2a7;
const SCCP_INVALIDATE_INBOUND = 0x4a28c9d7;
const SCCP_REVALIDATE_INBOUND = 0x6c1e27b4;
const SCCP_MINT_FROM_VERIFIER = 0x23e4c1a0;
const SCCP_BURN_TO_DOMAIN = 0x4f80d7e1;
const SCCP_VERIFIER_INITIALIZE = 0x35f2bca1;
const SCCP_VERIFIER_SUBMIT_SIGNATURE_COMMITMENT = 0x6a4df0b3;
const SCCP_VERIFIER_MINT_FROM_SORA_PROOF = 0x1a9b2c7d;
const SCCP_VERIFIER_MINT_FROM_SORA_PROOF_V2 = 0x1a9b2c7e;

// SCCP domains (must match other repos + SORA pallet).
const DOMAIN_SORA = 0;
const DOMAIN_ETH = 1;
const DOMAIN_TON = 4;

// Errors from `contracts/errors.tolk`.
const ERROR_NOT_OWNER = 73;
const ERROR_SCCP_DOMAIN_UNSUPPORTED = 1000;
const ERROR_SCCP_INBOUND_PAUSED = 1001;
const ERROR_SCCP_MESSAGE_INVALIDATED = 1002;
const ERROR_SCCP_MESSAGE_ALREADY_PROCESSED = 1003;
const ERROR_SCCP_VERIFIER_NOT_SET = 1004;
const ERROR_SCCP_NOT_VERIFIER = 1005;
const ERROR_SCCP_RECIPIENT_IS_ZERO = 1008;
const ERROR_SCCP_RECIPIENT_NOT_CANONICAL = 1009;
const ERROR_SCCP_UNKNOWN_MMR_ROOT = 1010;
const ERROR_SCCP_COMMITMENT_NOT_FOUND = 1012;
const ERROR_SCCP_VERIFIER_ALREADY_INITIALIZED = 1014;
const ERROR_SCCP_VERIFIER_NOT_INITIALIZED = 1015;
const ERROR_SCCP_COMMITMENT_TOO_OLD = 1016;
const ERROR_SCCP_INVALID_VALIDATOR_SET_ID = 1017;
const ERROR_SCCP_NOT_ENOUGH_VALIDATOR_SIGNATURES = 1018;
const ERROR_SCCP_INVALID_VALIDATOR_PROOF = 1019;
const ERROR_SCCP_INVALID_SIGNATURE = 1020;
const ERROR_SCCP_OUTBOUND_PAUSED = 1021;
const ERROR_SCCP_ADMIN_PATH_DISABLED = 1022;
const SECP256K1N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141n;

function loadArtifact(name) {
  return JSON.parse(readFileSync(resolve(repoRoot, 'artifacts', name), 'utf8'));
}

function codeFromArtifact(artifact) {
  return Cell.fromBoc(Buffer.from(artifact.codeBoc64, 'base64'))[0];
}

function addressToU256(addr) {
  return BigInt('0x' + addr.hash.toString('hex'));
}

function txExitCode(tx) {
  if (tx.description?.computePhase?.type === 'vm') {
    return tx.description.computePhase.exitCode;
  }
  return null;
}

function findTxByAddress(txs, addr) {
  const target = BigInt('0x' + addr.hash.toString('hex'));
  return txs.find((t) => t.address === target);
}

function buildMasterData({ governor, verifier, walletCode, metadataUri, soraAssetIdU256 }) {
  const emptyBoolMap = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Bool());
  const emptyBurnsMap = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Cell());

  const sccpExtraB = beginCell();
  sccpExtraB.storeUint(soraAssetIdU256, 256);
  sccpExtraB.storeUint(0, 64); // nonce
  sccpExtraB.storeUint(0, 64); // inboundPausedMask
  sccpExtraB.storeUint(0, 64); // outboundPausedMask
  emptyBoolMap.store(sccpExtraB); // invalidatedInbound
  emptyBoolMap.store(sccpExtraB); // processedInbound
  emptyBurnsMap.store(sccpExtraB); // burns
  const sccpExtra = sccpExtraB.endCell();

  const metadataCell = beginCell()
    .storeBuffer(Buffer.from(metadataUri ?? '', 'utf8'))
    .endCell();

  return beginCell()
    .storeCoins(0n) // totalSupply
    .storeAddress(governor)
    .storeAddress(verifier ?? null)
    .storeRef(walletCode)
    .storeRef(metadataCell)
    .storeRef(sccpExtra)
    .endCell();
}

class SccpJettonMaster {
  constructor(address, init) {
    this.address = address;
    this.init = init;
  }

  static createFromArtifacts(masterCode, walletCode, governor, soraAssetIdU256, workchain = 0) {
    const data = buildMasterData({
      governor,
      verifier: null,
      walletCode,
      metadataUri: '',
      soraAssetIdU256,
    });
    const init = { code: masterCode, data };
    return new SccpJettonMaster(contractAddress(workchain, init), init);
  }

  async sendDeploy(provider, via, value) {
    await provider.internal(via, {
      value,
      sendMode: SendMode.PAY_GAS_SEPARATELY,
      body: beginCell().storeUint(TOP_UP_TONS, 32).endCell(),
    });
  }

  async sendSetVerifier(provider, via, value, newVerifier) {
    const body = beginCell()
      .storeUint(SCCP_SET_VERIFIER, 32)
      .storeUint(0, 64)
      .storeAddress(newVerifier ?? null)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async sendSetInboundPaused(provider, via, value, sourceDomain, paused) {
    const body = beginCell()
      .storeUint(SCCP_SET_INBOUND_PAUSED, 32)
      .storeUint(0, 64)
      .storeUint(sourceDomain, 32)
      .storeBit(paused)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async sendSetOutboundPaused(provider, via, value, destDomain, paused) {
    const body = beginCell()
      .storeUint(SCCP_SET_OUTBOUND_PAUSED, 32)
      .storeUint(0, 64)
      .storeUint(destDomain, 32)
      .storeBit(paused)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async sendInvalidateInbound(provider, via, value, messageIdU256) {
    const body = beginCell()
      .storeUint(SCCP_INVALIDATE_INBOUND, 32)
      .storeUint(0, 64)
      .storeUint(messageIdU256, 256)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async sendRevalidateInbound(provider, via, value, messageIdU256) {
    const body = beginCell()
      .storeUint(SCCP_REVALIDATE_INBOUND, 32)
      .storeUint(0, 64)
      .storeUint(messageIdU256, 256)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async sendMintFromVerifier(provider, via, value, { sourceDomain, burnNonce, jettonAmount, recipient32 }) {
    const body = beginCell()
      .storeUint(SCCP_MINT_FROM_VERIFIER, 32)
      .storeUint(0, 64)
      .storeUint(sourceDomain, 32)
      .storeUint(burnNonce, 64)
      .storeCoins(jettonAmount)
      .storeUint(recipient32, 256)
      .storeAddress(null) // sendExcessesTo
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async getSccpConfig(provider) {
    const res = await provider.get('get_sccp_config');
    const governor = res.stack.readAddress();
    const verifier = res.stack.readAddressOpt();
    const soraAssetId = res.stack.readBigNumber();
    const nonce = res.stack.readBigNumber();
    const inboundPausedMask = res.stack.readBigNumber();
    const outboundPausedMask = res.stack.readBigNumber();
    return { governor, verifier, soraAssetId, nonce, inboundPausedMask, outboundPausedMask };
  }

  async getWalletAddress(provider, owner) {
    const arg = beginCell().storeAddress(owner).endCell();
    const res = await provider.get('get_wallet_address', [{ type: 'slice', cell: arg }]);
    return res.stack.readAddress();
  }

  async getOutboundMessageId(provider, destDomain, nonce, jettonAmount, recipient32) {
    const res = await provider.get('get_sccp_message_id', [
      { type: 'int', value: BigInt(destDomain) },
      { type: 'int', value: BigInt(nonce) },
      { type: 'int', value: BigInt(jettonAmount) },
      { type: 'int', value: BigInt(recipient32) },
    ]);
    return res.stack.readBigNumber();
  }

  async getInboundMessageId(provider, sourceDomain, burnNonce, jettonAmount, recipient32) {
    const res = await provider.get('get_sccp_inbound_message_id', [
      { type: 'int', value: BigInt(sourceDomain) },
      { type: 'int', value: BigInt(burnNonce) },
      { type: 'int', value: BigInt(jettonAmount) },
      { type: 'int', value: BigInt(recipient32) },
    ]);
    return res.stack.readBigNumber();
  }

  async getBurnRecord(provider, messageIdU256) {
    const res = await provider.get('get_sccp_burn_record', [{ type: 'int', value: BigInt(messageIdU256) }]);
    return res.stack.readCellOpt();
  }
}

class SccpJettonWallet {
  constructor(address, init) {
    this.address = address;
    this.init = init;
  }

  async sendSccpBurnToDomain(provider, via, value, { jettonAmount, destDomain, recipient32 }) {
    const body = beginCell()
      .storeUint(SCCP_BURN_TO_DOMAIN, 32)
      .storeUint(0, 64)
      .storeCoins(jettonAmount)
      .storeUint(destDomain, 32)
      .storeUint(recipient32, 256)
      .storeAddress(null) // sendExcessesTo
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async getWalletData(provider) {
    const res = await provider.get('get_wallet_data');
    const jettonBalance = res.stack.readBigNumber();
    const ownerAddress = res.stack.readAddress();
    const minterAddress = res.stack.readAddress();
    const jettonWalletCode = res.stack.readCell();
    return { jettonBalance, ownerAddress, minterAddress, jettonWalletCode };
  }
}

function buildVerifierData({ governor, jettonMaster, soraAssetIdU256 }) {
  const emptyMmrRootsMap = Dictionary.empty(Dictionary.Keys.Uint(16), Dictionary.Values.BigUint(256));
  const emptyKnownRootsMap = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Bool());

  // LightClientState (stored as a ref in VerifierStorage).
  const stB = beginCell();
  stB.storeBit(0); // initialized
  stB.storeUint(0, 64); // latestBeefyBlock
  // currentValidatorSet
  stB.storeUint(0, 64); // id
  stB.storeUint(0, 32); // len
  stB.storeUint(0n, 256); // root
  // nextValidatorSet
  stB.storeUint(0, 64); // id
  stB.storeUint(0, 32); // len
  stB.storeUint(0n, 256); // root
  stB.storeUint(0, 16); // mmrRootsPos
  emptyMmrRootsMap.store(stB); // mmrRoots ring
  emptyKnownRootsMap.store(stB); // knownRoots
  const stCell = stB.endCell();

  return beginCell()
    .storeAddress(governor)
    .storeAddress(jettonMaster)
    .storeUint(soraAssetIdU256, 256)
    .storeRef(stCell)
    .endCell();
}

function u256ToBufferBE(v) {
  const hex = v.toString(16).padStart(64, '0');
  return Buffer.from(hex, 'hex');
}

function buildSoraLeafProofWithDigest({ digestScaleBytes, nextAuthoritySetId, nextAuthoritySetLen, nextAuthoritySetRootU256 }) {
  // Proof format expected by `contracts/sccp-sora-verifier.tolk` (Substrate MMR single-leaf proof).
  // Keep it minimal: 1-leaf MMR => root == leafHash, no proof items.
  const itemsRef = beginCell().storeUint(0, 16).endCell(); // totalCount = 0
  const digestRef = beginCell().storeBuffer(digestScaleBytes).endCell();

  // Leaf fields are stored in a separate cell to avoid 1023-bit overflow when adding (leafIndex, leafCount).
  const leafRef = beginCell()
    .storeUint(0, 8) // leafVersion
    .storeUint(0, 32) // parentNumber
    .storeUint(0n, 256) // parentHash
    .storeUint(nextAuthoritySetId, 64)
    .storeUint(nextAuthoritySetLen, 32)
    .storeUint(nextAuthoritySetRootU256, 256)
    .storeUint(0n, 256) // randomSeed
    .storeRef(digestRef)
    .endCell();

  return beginCell()
    .storeUint(0, 64) // leafIndex
    .storeUint(1, 64) // leafCount
    .storeRef(itemsRef)
    .storeRef(leafRef)
    .endCell();
}

class SccpSoraVerifier {
  constructor(address, init) {
    this.address = address;
    this.init = init;
  }

  static createFromArtifacts(verifierCode, governor, jettonMaster, soraAssetIdU256, workchain = 0) {
    const data = buildVerifierData({ governor, jettonMaster, soraAssetIdU256 });
    const init = { code: verifierCode, data };
    return new SccpSoraVerifier(contractAddress(workchain, init), init);
  }

  async sendDeploy(provider, via, value) {
    await provider.internal(via, {
      value,
      sendMode: SendMode.PAY_GAS_SEPARATELY,
      body: beginCell().storeUint(TOP_UP_TONS, 32).endCell(),
    });
  }

  async sendInitialize(provider, via, value, {
    latestBeefyBlock,
    currentValidatorSetId,
    currentValidatorSetLen,
    currentValidatorSetRootU256,
    nextValidatorSetId,
    nextValidatorSetLen,
    nextValidatorSetRootU256,
  }) {
    const body = beginCell()
      .storeUint(SCCP_VERIFIER_INITIALIZE, 32)
      .storeUint(0, 64)
      .storeUint(latestBeefyBlock, 64)
      .storeUint(currentValidatorSetId, 64)
      .storeUint(currentValidatorSetLen, 32)
      .storeUint(currentValidatorSetRootU256, 256)
      .storeUint(nextValidatorSetId, 64)
      .storeUint(nextValidatorSetLen, 32)
      .storeUint(nextValidatorSetRootU256, 256)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async sendSubmitSignatureCommitment(provider, via, value, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId,
    validatorProofCell,
    latestLeafProofCell,
  }) {
    const body = beginCell()
      .storeUint(SCCP_VERIFIER_SUBMIT_SIGNATURE_COMMITMENT, 32)
      .storeUint(0, 64)
      .storeUint(commitmentMmrRootU256, 256)
      .storeUint(commitmentBlockNumber, 32)
      .storeUint(commitmentValidatorSetId, 64)
      .storeRef(validatorProofCell)
      .storeRef(latestLeafProofCell)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async sendMintFromSoraProof(provider, via, value, { burnNonce, jettonAmount, recipient32, proofCell }) {
    const body = beginCell()
      .storeUint(SCCP_VERIFIER_MINT_FROM_SORA_PROOF, 32)
      .storeUint(0, 64)
      .storeUint(burnNonce, 64)
      .storeCoins(jettonAmount)
      .storeUint(recipient32, 256)
      .storeAddress(null) // sendExcessesTo
      .storeRef(proofCell)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }

  async sendMintFromSoraProofV2(provider, via, value, { sourceDomain, burnNonce, jettonAmount, recipient32, proofCell }) {
    const body = beginCell()
      .storeUint(SCCP_VERIFIER_MINT_FROM_SORA_PROOF_V2, 32)
      .storeUint(0, 64)
      .storeUint(sourceDomain, 32)
      .storeUint(burnNonce, 64)
      .storeCoins(jettonAmount)
      .storeUint(recipient32, 256)
      .storeAddress(null) // sendExcessesTo
      .storeRef(proofCell)
      .endCell();
    await provider.internal(via, { value, sendMode: SendMode.PAY_GAS_SEPARATELY, body });
  }
}

test('SCCP Jetton master is fail-closed until verifier is set', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const verifier = await blockchain.treasury('verifier');
  const alice = await blockchain.treasury('alice');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);

  const aliceRecipient32 = addressToU256(alice.address);
  const out = await master.sendMintFromVerifier(verifier.getSender(), 1_000_000_000n, {
    sourceDomain: DOMAIN_SORA,
    burnNonce: 1n,
    jettonAmount: 10n,
    recipient32: aliceRecipient32,
  });

  const tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_VERIFIER_NOT_SET);
});

test('SCCP Jetton master local admin operations are disabled', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const verifier = await blockchain.treasury('verifier');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);

  let out = await master.sendSetVerifier(alice.getSender(), 1_000_000_000n, verifier.address);
  let tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for set verifier');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);

  out = await master.sendSetInboundPaused(alice.getSender(), 1_000_000_000n, DOMAIN_SORA, true);
  tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for set inbound paused');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);

  out = await master.sendSetOutboundPaused(alice.getSender(), 1_000_000_000n, DOMAIN_ETH, true);
  tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for set outbound paused');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);

  out = await master.sendInvalidateInbound(
    alice.getSender(),
    1_000_000_000n,
    BigInt('0x' + 'aa'.repeat(32)),
  );
  tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for invalidate');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);

  out = await master.sendRevalidateInbound(
    alice.getSender(),
    1_000_000_000n,
    BigInt('0x' + 'aa'.repeat(32)),
  );
  tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for revalidate');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);
});

test('SCCP Jetton master accepts mint-from-verifier only from configured verifier account', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const verifier = await blockchain.treasury('verifier');
  const alice = await blockchain.treasury('alice');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);
  await master.sendSetVerifier(verifier.getSender(), 1_000_000_000n, verifier.address);

  const aliceRecipient32 = addressToU256(alice.address);
  const out = await master.sendMintFromVerifier(alice.getSender(), 1_000_000_000n, {
    sourceDomain: DOMAIN_SORA,
    burnNonce: 1n,
    jettonAmount: 10n,
    recipient32: aliceRecipient32,
  });

  const tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_NOT_VERIFIER);
});

test('SCCP Jetton master refuses verifier reassignment after bootstrap', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const verifier = await blockchain.treasury('verifier');
  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);
  await master.sendSetVerifier(verifier.getSender(), 1_000_000_000n, verifier.address);
  const out = await master.sendSetVerifier(governor.getSender(), 1_000_000_000n, null);

  const tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);
});

test('SCCP Jetton master rejects zero recipient in verifier mint path', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const verifier = await blockchain.treasury('verifier');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);
  await master.sendSetVerifier(verifier.getSender(), 1_000_000_000n, verifier.address);

  const out = await master.sendMintFromVerifier(verifier.getSender(), 1_000_000_000n, {
    sourceDomain: DOMAIN_SORA,
    burnNonce: 9n,
    jettonAmount: 1n,
    recipient32: 0n,
  });
  const tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_RECIPIENT_IS_ZERO);
});

test('SCCP Jetton master rejects unsupported verifier source domains', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const verifier = await blockchain.treasury('verifier');
  const alice = await blockchain.treasury('alice');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);
  await master.sendSetVerifier(verifier.getSender(), 1_000_000_000n, verifier.address);

  const aliceRecipient32 = addressToU256(alice.address);
  const out = await master.sendMintFromVerifier(verifier.getSender(), 1_000_000_000n, {
    sourceDomain: 99,
    burnNonce: 1n,
    jettonAmount: 10n,
    recipient32: aliceRecipient32,
  });

  const tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_DOMAIN_UNSUPPORTED);
});

test('SCCP Jetton master rejects local verifier source domain', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const verifier = await blockchain.treasury('verifier');
  const alice = await blockchain.treasury('alice');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);
  await master.sendSetVerifier(verifier.getSender(), 1_000_000_000n, verifier.address);

  const aliceRecipient32 = addressToU256(alice.address);
  const out = await master.sendMintFromVerifier(verifier.getSender(), 1_000_000_000n, {
    sourceDomain: DOMAIN_TON,
    burnNonce: 3n,
    jettonAmount: 10n,
    recipient32: aliceRecipient32,
  });

  const tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_DOMAIN_UNSUPPORTED);
});

test('SCCP Jetton master rejects disabled local admin controls', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const verifier = await blockchain.treasury('verifier');
  const alice = await blockchain.treasury('alice');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);
  await master.sendSetVerifier(verifier.getSender(), 1_000_000_000n, verifier.address);

  let out = await master.sendSetInboundPaused(governor.getSender(), 1_000_000_000n, DOMAIN_TON, true);
  let tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for local-domain inbound pause');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);

  out = await master.sendSetOutboundPaused(governor.getSender(), 1_000_000_000n, DOMAIN_TON, true);
  tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for local-domain outbound pause');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);

  const aliceRecipient32 = addressToU256(alice.address);
  const burnNonce = 77n;
  const jettonAmount = 5n;
  const inboundMessageId = await master.getInboundMessageId(
    DOMAIN_SORA,
    burnNonce,
    jettonAmount,
    aliceRecipient32,
  );
  out = await master.sendInvalidateInbound(governor.getSender(), 1_000_000_000n, inboundMessageId);
  tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for invalidation');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);

  out = await master.sendRevalidateInbound(governor.getSender(), 1_000_000_000n, inboundMessageId);
  tx = findTxByAddress(out.transactions, master.address);
  assert.ok(tx, 'expected a master tx for revalidation');
  assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);
});

test('SCCP verifier V2 rejects unsupported source domains', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  // Initialize light client so V2 path reaches domain guard.
  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });

  const proofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: Buffer.from([0x00]), // empty digest vec
    nextAuthoritySetId: 2n,
    nextAuthoritySetLen: 1,
    nextAuthoritySetRootU256: 0n,
  });

  const out = await verifier.sendMintFromSoraProofV2(alice.getSender(), 1_000_000_000n, {
    sourceDomain: 99,
    burnNonce: 1n,
    jettonAmount: 1n,
    recipient32: BigInt(1),
    proofCell,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_DOMAIN_UNSUPPORTED);
});

test('SCCP verifier V2 rejects proofs before initialization', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  const proofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: Buffer.from([0x00]),
    nextAuthoritySetId: 0n,
    nextAuthoritySetLen: 0,
    nextAuthoritySetRootU256: 0n,
  });

  const out = await verifier.sendMintFromSoraProofV2(alice.getSender(), 1_000_000_000n, {
    sourceDomain: DOMAIN_SORA,
    burnNonce: 1n,
    jettonAmount: 1n,
    recipient32: BigInt(1),
    proofCell,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_VERIFIER_NOT_INITIALIZED);
});

test('SCCP verifier rejects submit-signature-commitment before initialization', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  const emptyProof = beginCell().endCell();
  const out = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256: 0n,
    commitmentBlockNumber: 1,
    commitmentValidatorSetId: 1n,
    validatorProofCell: emptyProof,
    latestLeafProofCell: emptyProof,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_VERIFIER_NOT_INITIALIZED);
});

test('SCCP verifier initialize is permissionless once', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  const out = await verifier.sendInitialize(alice.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), 0);
});

test('SCCP verifier initialize self-registers the jetton master once', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);

  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, master.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  await verifier.sendInitialize(alice.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });

  const cfg = await master.getSccpConfig();
  assert.equal(cfg.verifier?.toString(), verifier.address.toString());
});

test('SCCP verifier initialize is one-time', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });

  const out = await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_VERIFIER_ALREADY_INITIALIZED);
});

test('SCCP verifier submit-signature-commitment enforces freshness and validator-set id guards', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);
  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 10,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });

  const emptyProof = beginCell().endCell();

  const staleOut = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256: 0n,
    commitmentBlockNumber: 10,
    commitmentValidatorSetId: 1n,
    validatorProofCell: emptyProof,
    latestLeafProofCell: emptyProof,
  });
  let tx = findTxByAddress(staleOut.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx for stale commitment');
  assert.equal(txExitCode(tx), ERROR_SCCP_COMMITMENT_TOO_OLD);

  const wrongSetOut = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256: 0n,
    commitmentBlockNumber: 11,
    commitmentValidatorSetId: 999n,
    validatorProofCell: emptyProof,
    latestLeafProofCell: emptyProof,
  });
  tx = findTxByAddress(wrongSetOut.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx for invalid validator-set id');
  assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_SET_ID);
});

test('SCCP verifier submit-signature-commitment rejects insufficient validator signatures', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);
  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });

  const noSignaturesProof = beginCell().storeUint(0, 16).endCell();
  const emptyLeafProof = beginCell().endCell();
  const out = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256: 0n,
    commitmentBlockNumber: 1,
    commitmentValidatorSetId: 1n,
    validatorProofCell: noSignaturesProof,
    latestLeafProofCell: emptyLeafProof,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_NOT_ENOUGH_VALIDATOR_SIGNATURES);
});

test('SCCP verifier submit-signature-commitment enforces 2-of-2 threshold for tiny validator sets', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);
  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 2,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 2,
    nextValidatorSetRootU256: 0n,
  });

  // n=1 for setLen=2 should fail the >=2/3 threshold check before parsing entries.
  const oneSignatureProof = beginCell().storeUint(1, 16).endCell();
  const emptyLeafProof = beginCell().endCell();
  const out = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256: 0n,
    commitmentBlockNumber: 1,
    commitmentValidatorSetId: 1n,
    validatorProofCell: oneSignatureProof,
    latestLeafProofCell: emptyLeafProof,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_NOT_ENOUGH_VALIDATOR_SIGNATURES);
});

test('SCCP verifier submit-signature-commitment rejects signature count above validator-set length', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);
  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });

  // n=2 for setLen=1 must fail closed before any signature parsing.
  const overSubscribedProof = beginCell().storeUint(2, 16).endCell();
  const emptyLeafProof = beginCell().endCell();
  const out = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256: 0n,
    commitmentBlockNumber: 1,
    commitmentValidatorSetId: 1n,
    validatorProofCell: overSubscribedProof,
    latestLeafProofCell: emptyLeafProof,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_PROOF);
});

test('SCCP verifier submit-signature-commitment rejects zero-length validator sets', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);
  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 0,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 0,
    nextValidatorSetRootU256: 0n,
  });

  const noSignaturesProof = beginCell().storeUint(0, 16).endCell();
  const emptyLeafProof = beginCell().endCell();
  const out = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256: 0n,
    commitmentBlockNumber: 1,
    commitmentValidatorSetId: 1n,
    validatorProofCell: noSignaturesProof,
    latestLeafProofCell: emptyLeafProof,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_PROOF);
});

test('SCCP verifier V2 rejects local TON source domain', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);
  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId: 1n,
    currentValidatorSetLen: 1,
    currentValidatorSetRootU256: 0n,
    nextValidatorSetId: 2n,
    nextValidatorSetLen: 1,
    nextValidatorSetRootU256: 0n,
  });

  const proofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: Buffer.from([0x00]),
    nextAuthoritySetId: 2n,
    nextAuthoritySetLen: 1,
    nextAuthoritySetRootU256: 0n,
  });

  const out = await verifier.sendMintFromSoraProofV2(alice.getSender(), 1_000_000_000n, {
    sourceDomain: DOMAIN_TON,
    burnNonce: 1n,
    jettonAmount: 1n,
    recipient32: BigInt(1),
    proofCell,
  });
  const tx = findTxByAddress(out.transactions, verifier.address);
  assert.ok(tx, 'expected a verifier tx');
  assert.equal(txExitCode(tx), ERROR_SCCP_DOMAIN_UNSUPPORTED);
});

test('SCCP Jetton flow: mint (verifier-gated), replay blocked, admin paths disabled, burn record stored', async () => {
  const masterArtifact = loadArtifact('sccp-jetton-master.compiled.json');
  const walletArtifact = loadArtifact('sccp-jetton-wallet.compiled.json');
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const masterCode = codeFromArtifact(masterArtifact);
  const walletCode = codeFromArtifact(walletArtifact);
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const master = blockchain.openContract(
    SccpJettonMaster.createFromArtifacts(masterCode, walletCode, governor.address, soraAssetIdU256),
  );
  await master.sendDeploy(governor.getSender(), 1_000_000_000n);

  const aliceRecipient32 = addressToU256(alice.address);
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, master.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  // Build a single SORA "leaf provider" digest that commits to multiple SCCP messageIds.
  const mintNonce1 = 777n;
  const mintAmount1 = 1000n;
  const mintNonceEth = 666n;
  const mintAmountEth = 7n;
  const mintNonce2 = 888n;
  const mintAmount2 = 1n;

  const msgId1 = await master.getInboundMessageId(DOMAIN_SORA, mintNonce1, mintAmount1, aliceRecipient32);
  const msgIdEth = await master.getInboundMessageId(DOMAIN_ETH, mintNonceEth, mintAmountEth, aliceRecipient32);
  const msgId2 = await master.getInboundMessageId(DOMAIN_SORA, mintNonce2, mintAmount2, aliceRecipient32);

  function buildDigestScaleForMessageIds(messageIds) {
    // SCALE Vec<AuxiliaryDigestItem>, where we only support AuxiliaryDigestItem::Commitment(GenericNetworkId::EVMLegacy(u32), H256).
    // Compact length (mode 0) for small n: first byte = n * 4.
    const n = messageIds.length;
    assert.ok(n > 0 && n < 64, 'n out of range');

    const out = [];
    out.push(n * 4); // compact u32 (mode 0)
    for (const m of messageIds) {
      out.push(0x00); // AuxiliaryDigestItem::Commitment
      out.push(0x02); // GenericNetworkId::EVMLegacy
      out.push(0x50, 0x43, 0x43, 0x53); // u32 LE of 0x53434350 ('SCCP')
      out.push(...u256ToBufferBE(m));
    }
    return Buffer.from(out);
  }

  const digestScale = buildDigestScaleForMessageIds([msgId1, msgIdEth, msgId2]);
  const digestHash32 = Buffer.from(ethers.keccak256(digestScale).slice(2), 'hex');

  // --- Synthetic validator set (BEEFY) ---
  //
  // Build a 4-validator set so >=2/3 threshold is 3 signatures.
  const validatorPrivKeys = [
    '0x' + '11'.repeat(32),
    '0x' + '22'.repeat(32),
    '0x' + '33'.repeat(32),
    '0x' + '44'.repeat(32),
  ];

  const validators = validatorPrivKeys.map((pk) => {
    const w = new ethers.Wallet(pk);
    const addr160 = BigInt(w.address);
    const addr20 = Buffer.from(w.address.slice(2).padStart(40, '0'), 'hex');
    return { pk, address: w.address, addr160, addr20 };
  }).sort((a, b) => (a.addr160 < b.addr160 ? -1 : 1));

  function keccak256Buf(data) {
    return Buffer.from(ethers.keccak256(data).slice(2), 'hex');
  }

  function keccakPair(a32, b32) {
    // Substrate `binary_merkle_tree`: ordered hashing (no sorting).
    return keccak256Buf(Buffer.concat([a32, b32]));
  }

  function buildMerkleRootAndProofs(leaves) {
    let level = leaves.slice();
    const levels = [level];
    while (level.length > 1) {
      const next = [];
      for (let i = 0; i < level.length; i += 2) {
        if (i + 1 >= level.length) {
          next.push(level[i]); // promote odd leaf
        } else {
          next.push(keccakPair(level[i], level[i + 1]));
        }
      }
      level = next;
      levels.push(level);
    }

    const root = levels[levels.length - 1][0];

    const proofs = leaves.map((_leaf, leafIndex) => {
      const out = [];
      let idx = leafIndex;
      for (let d = 0; d < levels.length - 1; d++) {
        const layer = levels[d];
        const sib = idx % 2 === 1 ? idx - 1 : idx + 1;
        if (sib < layer.length) {
          out.push(layer[sib]);
        }
        idx = Math.floor(idx / 2);
      }
      return out;
    });

    return { root, proofs };
  }

  const leaves = validators.map((v) => keccak256Buf(v.addr20));
  const { root: validatorSetRoot32, proofs: validatorProofs32 } = buildMerkleRootAndProofs(leaves);
  const validatorSetRootU256 = BigInt('0x' + validatorSetRoot32.toString('hex'));

  const currentValidatorSetId = 1n;
  const nextValidatorSetId = 2n;
  const validatorSetLen = 4;

  // Burn proof leaf must advertise the next validator set (we keep it equal to `next` so no rotation happens).
  const nextAuthoritySetId = nextValidatorSetId;
  const nextAuthoritySetLen = validatorSetLen;
  const nextAuthoritySetRootU256 = validatorSetRootU256;

  const proofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: digestScale,
    nextAuthoritySetId,
    nextAuthoritySetLen,
    nextAuthoritySetRootU256,
  });

  // Verifier bootstrap must happen once before any proof-backed operation.
  const mintNotInit = await verifier.sendMintFromSoraProof(alice.getSender(), 1_000_000_000n, {
    burnNonce: mintNonce1,
    jettonAmount: mintAmount1,
    recipient32: aliceRecipient32,
    proofCell,
  });
  {
    const tx = findTxByAddress(mintNotInit.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_VERIFIER_NOT_INITIALIZED);
  }

  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId,
    currentValidatorSetLen: validatorSetLen,
    currentValidatorSetRootU256: validatorSetRootU256,
    nextValidatorSetId,
    nextValidatorSetLen: validatorSetLen,
    nextValidatorSetRootU256: validatorSetRootU256,
  });

  // Root is unknown until a valid commitment is submitted.
  const mintUnknownRoot = await verifier.sendMintFromSoraProof(alice.getSender(), 1_000_000_000n, {
    burnNonce: mintNonce1,
    jettonAmount: mintAmount1,
    recipient32: aliceRecipient32,
    proofCell,
  });
  {
    const tx = findTxByAddress(mintUnknownRoot.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_UNKNOWN_MMR_ROOT);
  }

  // --- Submit a synthetic BEEFY commitment that imports the proof root as a known MMR root ---
  const leafScale = Buffer.alloc(145);
  leafScale[0] = 0; // version
  leafScale.writeUInt32LE(0, 1); // parentNumber
  // parentHash[32] @ 5..37 => already zero
  leafScale.writeBigUInt64LE(nextAuthoritySetId, 37);
  leafScale.writeUInt32LE(nextAuthoritySetLen, 45);
  Buffer.from(nextAuthoritySetRootU256.toString(16).padStart(64, '0'), 'hex').copy(leafScale, 49);
  // randomSeed[32] @ 81..113 => zero
  digestHash32.copy(leafScale, 113);
  const mmrRoot32 = keccak256Buf(leafScale);
  const commitmentMmrRootU256 = BigInt('0x' + mmrRoot32.toString('hex'));

  const commitmentBlockNumber = 10;
  const commitmentValidatorSetId = currentValidatorSetId;

  const commitmentScale = Buffer.alloc(48);
  commitmentScale[0] = 0x04;
  commitmentScale[1] = 'm'.charCodeAt(0);
  commitmentScale[2] = 'h'.charCodeAt(0);
  commitmentScale[3] = 0x80;
  mmrRoot32.copy(commitmentScale, 4);
  commitmentScale.writeUInt32LE(commitmentBlockNumber, 36);
  commitmentScale.writeBigUInt64LE(commitmentValidatorSetId, 40);
  const commitmentHashHex = ethers.keccak256(commitmentScale);

  // Use 3 signatures (>=2/3 of 4) from the first 3 validators (already sorted by address).
  const signingValidators = validators.slice(0, 3).map((v, idx) => {
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    const v27 = 27 + sig.yParity;
    return {
      addr160: v.addr160,
      v: v27,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });

  function buildMerkleProofCell(siblings32) {
    const b = beginCell();
    b.storeUint(siblings32.length, 16);
    for (const sib of siblings32) {
      b.storeUint(BigInt('0x' + sib.toString('hex')), 256);
    }
    return b.endCell();
  }

  function buildValidatorProofCell(entries) {
    // Linked list: each entry cell holds one signature + merkle proof + maybeRef(next).
    let next = null;
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i];
      const merkleProofCell = buildMerkleProofCell(e.merkleProofSiblings32);
      next = beginCell()
        .storeUint(e.v, 8)
        .storeUint(e.r, 256)
        .storeUint(e.s, 256)
        .storeUint(e.pos, 32)
        .storeRef(merkleProofCell)
        .storeMaybeRef(next)
        .endCell();
    }

    return beginCell()
      .storeUint(entries.length, 16)
      .storeRef(next)
      .endCell();
  }

  const validatorProofCell = buildValidatorProofCell(signingValidators);

  const submitOut = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId,
    validatorProofCell,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitOut.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), 0);
  }

  // Mint to alice (must go through verifier contract + proof).
  const mint1 = await verifier.sendMintFromSoraProof(alice.getSender(), 1_000_000_000n, {
    burnNonce: mintNonce1,
    jettonAmount: mintAmount1,
    recipient32: aliceRecipient32,
    proofCell,
  });
  {
    const tx = findTxByAddress(mint1.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), 0);
  }

  // Mint from a non-SORA sourceDomain: ETH -> TON, attested/committed by SORA.
  const mintEth = await verifier.sendMintFromSoraProofV2(alice.getSender(), 1_000_000_000n, {
    sourceDomain: DOMAIN_ETH,
    burnNonce: mintNonceEth,
    jettonAmount: mintAmountEth,
    recipient32: aliceRecipient32,
    proofCell,
  });
  {
    const tx = findTxByAddress(mintEth.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), 0);
  }

  const walletAddr = await master.getWalletAddress(alice.address);
  const wallet = blockchain.openContract(new SccpJettonWallet(walletAddr));

  // Even if a messageId is present, digest SCALE with trailing garbage must fail-closed.
  const trailingNonce = 999n;
  const trailingAmount = 1n;
  const trailingMsgId = await master.getInboundMessageId(
    DOMAIN_SORA,
    trailingNonce,
    trailingAmount,
    aliceRecipient32,
  );
  const trailingDigestScale = Buffer.concat([
    buildDigestScaleForMessageIds([trailingMsgId]),
    Buffer.from([0x00]),
  ]);
  const trailingDigestHash32 = Buffer.from(
    ethers.keccak256(trailingDigestScale).slice(2),
    'hex',
  );
  const trailingProofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: trailingDigestScale,
    nextAuthoritySetId,
    nextAuthoritySetLen,
    nextAuthoritySetRootU256,
  });

  const trailingLeafScale = Buffer.alloc(145);
  trailingLeafScale[0] = 0;
  trailingLeafScale.writeUInt32LE(0, 1);
  trailingLeafScale.writeBigUInt64LE(nextAuthoritySetId, 37);
  trailingLeafScale.writeUInt32LE(nextAuthoritySetLen, 45);
  Buffer.from(nextAuthoritySetRootU256.toString(16).padStart(64, '0'), 'hex').copy(trailingLeafScale, 49);
  trailingDigestHash32.copy(trailingLeafScale, 113);
  const trailingMmrRoot32 = keccak256Buf(trailingLeafScale);
  const trailingCommitmentMmrRootU256 = BigInt('0x' + trailingMmrRoot32.toString('hex'));

  const trailingCommitmentBlockNumber = commitmentBlockNumber + 1;
  const trailingCommitmentScale = Buffer.alloc(48);
  trailingCommitmentScale[0] = 0x04;
  trailingCommitmentScale[1] = 'm'.charCodeAt(0);
  trailingCommitmentScale[2] = 'h'.charCodeAt(0);
  trailingCommitmentScale[3] = 0x80;
  trailingMmrRoot32.copy(trailingCommitmentScale, 4);
  trailingCommitmentScale.writeUInt32LE(trailingCommitmentBlockNumber, 36);
  trailingCommitmentScale.writeBigUInt64LE(commitmentValidatorSetId, 40);
  const trailingCommitmentHashHex = ethers.keccak256(trailingCommitmentScale);

  const trailingSigningEntries = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(trailingCommitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const trailingValidatorProofCell = buildValidatorProofCell(trailingSigningEntries);

  const submitTrailingOut = await verifier.sendSubmitSignatureCommitment(
    alice.getSender(),
    1_000_000_000n,
    {
      commitmentMmrRootU256: trailingCommitmentMmrRootU256,
      commitmentBlockNumber: trailingCommitmentBlockNumber,
      commitmentValidatorSetId,
      validatorProofCell: trailingValidatorProofCell,
      latestLeafProofCell: trailingProofCell,
    },
  );
  {
    const tx = findTxByAddress(submitTrailingOut.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), 0);
  }

  const walletBeforeTrailingMint = await wallet.getWalletData();
  const trailingMint = await verifier.sendMintFromSoraProof(
    alice.getSender(),
    1_000_000_000n,
    {
      burnNonce: trailingNonce,
      jettonAmount: trailingAmount,
      recipient32: aliceRecipient32,
      proofCell: trailingProofCell,
    },
  );
  {
    const tx = findTxByAddress(trailingMint.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_COMMITMENT_NOT_FOUND);
  }
  const walletAfterTrailingMint = await wallet.getWalletData();
  assert.equal(
    walletAfterTrailingMint.jettonBalance,
    walletBeforeTrailingMint.jettonBalance,
  );

  // Compact mode=3 digest vector length encoding must fail closed.
  const mode3Nonce = 1000n;
  const mode3Amount = 1n;
  const mode3MsgId = await master.getInboundMessageId(
    DOMAIN_SORA,
    mode3Nonce,
    mode3Amount,
    aliceRecipient32,
  );
  const mode3DigestScale = Buffer.from([0x03, 0x00, 0x00, 0x00, 0x00]);
  const mode3DigestHash32 = Buffer.from(
    ethers.keccak256(mode3DigestScale).slice(2),
    'hex',
  );
  const mode3ProofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: mode3DigestScale,
    nextAuthoritySetId,
    nextAuthoritySetLen,
    nextAuthoritySetRootU256,
  });

  const mode3LeafScale = Buffer.alloc(145);
  mode3LeafScale[0] = 0;
  mode3LeafScale.writeUInt32LE(0, 1);
  mode3LeafScale.writeBigUInt64LE(nextAuthoritySetId, 37);
  mode3LeafScale.writeUInt32LE(nextAuthoritySetLen, 45);
  Buffer.from(nextAuthoritySetRootU256.toString(16).padStart(64, '0'), 'hex').copy(mode3LeafScale, 49);
  mode3DigestHash32.copy(mode3LeafScale, 113);
  const mode3MmrRoot32 = keccak256Buf(mode3LeafScale);
  const mode3CommitmentMmrRootU256 = BigInt('0x' + mode3MmrRoot32.toString('hex'));

  const mode3CommitmentBlockNumber = trailingCommitmentBlockNumber + 1;
  const mode3CommitmentScale = Buffer.alloc(48);
  mode3CommitmentScale[0] = 0x04;
  mode3CommitmentScale[1] = 'm'.charCodeAt(0);
  mode3CommitmentScale[2] = 'h'.charCodeAt(0);
  mode3CommitmentScale[3] = 0x80;
  mode3MmrRoot32.copy(mode3CommitmentScale, 4);
  mode3CommitmentScale.writeUInt32LE(mode3CommitmentBlockNumber, 36);
  mode3CommitmentScale.writeBigUInt64LE(commitmentValidatorSetId, 40);
  const mode3CommitmentHashHex = ethers.keccak256(mode3CommitmentScale);

  const mode3SigningEntries = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(mode3CommitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const mode3ValidatorProofCell = buildValidatorProofCell(mode3SigningEntries);

  const submitMode3Out = await verifier.sendSubmitSignatureCommitment(
    alice.getSender(),
    1_000_000_000n,
    {
      commitmentMmrRootU256: mode3CommitmentMmrRootU256,
      commitmentBlockNumber: mode3CommitmentBlockNumber,
      commitmentValidatorSetId,
      validatorProofCell: mode3ValidatorProofCell,
      latestLeafProofCell: mode3ProofCell,
    },
  );
  {
    const tx = findTxByAddress(submitMode3Out.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), 0);
  }

  const walletBeforeMode3Mint = await wallet.getWalletData();
  const mode3Mint = await verifier.sendMintFromSoraProof(
    alice.getSender(),
    1_000_000_000n,
    {
      burnNonce: mode3Nonce,
      jettonAmount: mode3Amount,
      recipient32: aliceRecipient32,
      proofCell: mode3ProofCell,
    },
  );
  {
    const tx = findTxByAddress(mode3Mint.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_COMMITMENT_NOT_FOUND);
  }
  const walletAfterMode3Mint = await wallet.getWalletData();
  assert.equal(
    walletAfterMode3Mint.jettonBalance,
    walletBeforeMode3Mint.jettonBalance,
  );

  // Vec len=2 with a non-commitment second item must fail closed.
  const malformedKindNonce = 1001n;
  const malformedKindAmount = 1n;
  const malformedKindMsgId = await master.getInboundMessageId(
    DOMAIN_SORA,
    malformedKindNonce,
    malformedKindAmount,
    aliceRecipient32,
  );
  const firstItemOnlyDigest = buildDigestScaleForMessageIds([malformedKindMsgId]);
  const malformedKindDigestScale = Buffer.concat([
    Buffer.from([0x08]), // compact(vec len=2)
    firstItemOnlyDigest.subarray(1), // first commitment item (without old vec prefix)
    Buffer.from([0x01]), // invalid second item kind
  ]);
  const malformedKindDigestHash32 = Buffer.from(
    ethers.keccak256(malformedKindDigestScale).slice(2),
    'hex',
  );
  const malformedKindProofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: malformedKindDigestScale,
    nextAuthoritySetId,
    nextAuthoritySetLen,
    nextAuthoritySetRootU256,
  });

  const malformedKindLeafScale = Buffer.alloc(145);
  malformedKindLeafScale[0] = 0;
  malformedKindLeafScale.writeUInt32LE(0, 1);
  malformedKindLeafScale.writeBigUInt64LE(nextAuthoritySetId, 37);
  malformedKindLeafScale.writeUInt32LE(nextAuthoritySetLen, 45);
  Buffer.from(nextAuthoritySetRootU256.toString(16).padStart(64, '0'), 'hex').copy(malformedKindLeafScale, 49);
  malformedKindDigestHash32.copy(malformedKindLeafScale, 113);
  const malformedKindMmrRoot32 = keccak256Buf(malformedKindLeafScale);
  const malformedKindCommitmentMmrRootU256 = BigInt('0x' + malformedKindMmrRoot32.toString('hex'));

  const malformedKindCommitmentBlockNumber = mode3CommitmentBlockNumber + 1;
  const malformedKindCommitmentScale = Buffer.alloc(48);
  malformedKindCommitmentScale[0] = 0x04;
  malformedKindCommitmentScale[1] = 'm'.charCodeAt(0);
  malformedKindCommitmentScale[2] = 'h'.charCodeAt(0);
  malformedKindCommitmentScale[3] = 0x80;
  malformedKindMmrRoot32.copy(malformedKindCommitmentScale, 4);
  malformedKindCommitmentScale.writeUInt32LE(malformedKindCommitmentBlockNumber, 36);
  malformedKindCommitmentScale.writeBigUInt64LE(commitmentValidatorSetId, 40);
  const malformedKindCommitmentHashHex = ethers.keccak256(malformedKindCommitmentScale);

  const malformedKindSigningEntries = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(malformedKindCommitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const malformedKindValidatorProofCell = buildValidatorProofCell(malformedKindSigningEntries);

  const submitMalformedKindOut = await verifier.sendSubmitSignatureCommitment(
    alice.getSender(),
    1_000_000_000n,
    {
      commitmentMmrRootU256: malformedKindCommitmentMmrRootU256,
      commitmentBlockNumber: malformedKindCommitmentBlockNumber,
      commitmentValidatorSetId,
      validatorProofCell: malformedKindValidatorProofCell,
      latestLeafProofCell: malformedKindProofCell,
    },
  );
  {
    const tx = findTxByAddress(submitMalformedKindOut.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), 0);
  }

  const walletBeforeMalformedKindMint = await wallet.getWalletData();
  const malformedKindMint = await verifier.sendMintFromSoraProof(
    alice.getSender(),
    1_000_000_000n,
    {
      burnNonce: malformedKindNonce,
      jettonAmount: malformedKindAmount,
      recipient32: aliceRecipient32,
      proofCell: malformedKindProofCell,
    },
  );
  {
    const tx = findTxByAddress(malformedKindMint.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_COMMITMENT_NOT_FOUND);
  }
  const walletAfterMalformedKindMint = await wallet.getWalletData();
  assert.equal(
    walletAfterMalformedKindMint.jettonBalance,
    walletBeforeMalformedKindMint.jettonBalance,
  );

  const w0 = await wallet.getWalletData();
  assert.equal(w0.jettonBalance, mintAmount1 + mintAmountEth);
  assert.equal(w0.ownerAddress.toRawString(), alice.address.toRawString());
  assert.equal(w0.minterAddress.toRawString(), master.address.toRawString());

  // Replay the same inbound mint must be blocked.
  const mint2 = await verifier.sendMintFromSoraProof(alice.getSender(), 1_000_000_000n, {
    burnNonce: mintNonce1,
    jettonAmount: mintAmount1,
    recipient32: aliceRecipient32,
    proofCell,
  });
  {
    const tx = findTxByAddress(mint2.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_MESSAGE_ALREADY_PROCESSED);
  }

  // Replay the same inbound mint (ETH) must be blocked.
  const mintEthReplay = await verifier.sendMintFromSoraProofV2(alice.getSender(), 1_000_000_000n, {
    sourceDomain: DOMAIN_ETH,
    burnNonce: mintNonceEth,
    jettonAmount: mintAmountEth,
    recipient32: aliceRecipient32,
    proofCell,
  });
  {
    const tx = findTxByAddress(mintEthReplay.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_MESSAGE_ALREADY_PROCESSED);
  }

  // Disabled local admin controls must fail closed and leave proof-backed minting intact.
  const disableInbound = await master.sendSetInboundPaused(
    governor.getSender(),
    1_000_000_000n,
    DOMAIN_SORA,
    true,
  );
  {
    const tx = findTxByAddress(disableInbound.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);
  }

  const disableInvalidation = await master.sendInvalidateInbound(
    governor.getSender(),
    1_000_000_000n,
    msgId2,
  );
  {
    const tx = findTxByAddress(disableInvalidation.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);
  }

  const mint2Live = await verifier.sendMintFromSoraProof(alice.getSender(), 1_000_000_000n, {
    burnNonce: mintNonce2,
    jettonAmount: mintAmount2,
    recipient32: aliceRecipient32,
    proofCell,
  });
  {
    const tx = findTxByAddress(mint2Live.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), 0);
  }

  // Burn from TON -> SORA and verify burn record is stored on-chain (in master state).
  const cfg0 = await master.getSccpConfig();
  assert.equal(cfg0.nonce, 0n);

  // Disabled outbound controls must not create synthetic pause state.
  const disableOutbound = await master.sendSetOutboundPaused(
    governor.getSender(),
    1_000_000_000n,
    DOMAIN_SORA,
    true,
  );
  {
    const tx = findTxByAddress(disableOutbound.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);
  }
  const cfgAfterDisabledOutbound = await master.getSccpConfig();
  assert.equal(cfgAfterDisabledOutbound.nonce, cfg0.nonce);

  const burnAmount = 10n;
  const soraRecipient32 = BigInt('0x' + '22'.repeat(32));
  const burnOut = await wallet.sendSccpBurnToDomain(alice.getSender(), 1_000_000_000n, {
    jettonAmount: burnAmount,
    destDomain: DOMAIN_SORA,
    recipient32: soraRecipient32,
  });
  {
    const tx = findTxByAddress(burnOut.transactions, wallet.address);
    assert.ok(tx, 'expected a wallet tx');
    assert.equal(txExitCode(tx), 0);
  }

  const w1 = await wallet.getWalletData();
  assert.equal(w1.jettonBalance, mintAmount1 + mintAmountEth + mintAmount2 - burnAmount);

  const cfg = await master.getSccpConfig();
  assert.equal(cfg.nonce, 1n);

  const outMsgId = await master.getOutboundMessageId(DOMAIN_SORA, cfg.nonce, burnAmount, soraRecipient32);
  const burnCell = await master.getBurnRecord(outMsgId);
  assert.ok(burnCell, 'expected burn record to exist');

  const s = burnCell.beginParse();
  const burnInitiator = s.loadAddress();
  const destDomain = s.loadUint(32);
  const recipient32 = s.loadUintBig(256);
  const jettonAmount = s.loadCoins();
  const nonce = s.loadUintBig(64);

  assert.equal(burnInitiator.toRawString(), alice.address.toRawString());
  assert.equal(destDomain, DOMAIN_SORA);
  assert.equal(recipient32, soraRecipient32);
  assert.equal(jettonAmount, burnAmount);
  assert.equal(nonce, 1n);

  // Burn to an EVM domain must enforce canonical recipient encoding (high 12 bytes must be zero).
  const badEvmRecipient32 = BigInt('0x' + '11'.repeat(32)); // non-zero high 12 bytes => non-canonical
  const burnBad = await wallet.sendSccpBurnToDomain(alice.getSender(), 1_000_000_000n, {
    jettonAmount: 1n,
    destDomain: DOMAIN_ETH,
    recipient32: badEvmRecipient32,
  });
  {
    const tx = findTxByAddress(burnBad.transactions, wallet.address);
    assert.ok(tx, 'expected a wallet tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_RECIPIENT_NOT_CANONICAL);
  }
  const walletAfterBadEvmBurn = await wallet.getWalletData();
  assert.equal(walletAfterBadEvmBurn.jettonBalance, w1.jettonBalance);
  const cfgAfterBadEvmBurn = await master.getSccpConfig();
  assert.equal(cfgAfterBadEvmBurn.nonce, cfg.nonce);
  const badEvmAttemptMessageId = await master.getOutboundMessageId(
    DOMAIN_ETH,
    cfgAfterBadEvmBurn.nonce + 1n,
    1n,
    badEvmRecipient32,
  );
  const badEvmAttemptRecord = await master.getBurnRecord(badEvmAttemptMessageId);
  assert.equal(badEvmAttemptRecord, null);

  // Burn with an all-zero recipient must fail-closed and preserve wallet/master state.
  const walletBeforeZeroRecipientBurn = await wallet.getWalletData();
  const cfgBeforeZeroRecipientBurn = await master.getSccpConfig();
  const burnZeroRecipient = await wallet.sendSccpBurnToDomain(alice.getSender(), 1_000_000_000n, {
    jettonAmount: 1n,
    destDomain: DOMAIN_ETH,
    recipient32: 0n,
  });
  {
    const tx = findTxByAddress(burnZeroRecipient.transactions, wallet.address);
    assert.ok(tx, 'expected a wallet tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_RECIPIENT_IS_ZERO);
  }
  const walletAfterZeroRecipientBurn = await wallet.getWalletData();
  assert.equal(walletAfterZeroRecipientBurn.jettonBalance, walletBeforeZeroRecipientBurn.jettonBalance);
  const cfgAfterZeroRecipientBurn = await master.getSccpConfig();
  assert.equal(cfgAfterZeroRecipientBurn.nonce, cfgBeforeZeroRecipientBurn.nonce);

  // Burn to local TON domain must fail-closed and preserve wallet/master state.
  const walletBeforeLocalBurn = await wallet.getWalletData();
  const cfgBeforeLocalBurn = await master.getSccpConfig();
  const localBurnRecipient32 = BigInt('0x' + '22'.repeat(32));
  const burnLocalDomain = await wallet.sendSccpBurnToDomain(alice.getSender(), 1_000_000_000n, {
    jettonAmount: 1n,
    destDomain: DOMAIN_TON,
    recipient32: localBurnRecipient32,
  });
  {
    const tx = findTxByAddress(burnLocalDomain.transactions, wallet.address);
    assert.ok(tx, 'expected a wallet tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_DOMAIN_UNSUPPORTED);
  }
  const walletAfterLocalBurn = await wallet.getWalletData();
  assert.equal(walletAfterLocalBurn.jettonBalance, walletBeforeLocalBurn.jettonBalance);
  const cfgAfterLocalBurn = await master.getSccpConfig();
  assert.equal(cfgAfterLocalBurn.nonce, cfgBeforeLocalBurn.nonce);
  const localAttemptMessageId = await master.getOutboundMessageId(
    DOMAIN_TON,
    cfgBeforeLocalBurn.nonce + 1n,
    1n,
    localBurnRecipient32,
  );
  const localAttemptRecord = await master.getBurnRecord(localAttemptMessageId);
  assert.equal(localAttemptRecord, null);

  // Burn to an unsupported destination domain must fail-closed and preserve wallet/master state.
  const walletBeforeUnsupportedBurn = await wallet.getWalletData();
  const cfgBeforeUnsupportedBurn = await master.getSccpConfig();
  const unsupportedRecipient32 = BigInt('0x' + '22'.repeat(32));
  const burnUnsupported = await wallet.sendSccpBurnToDomain(alice.getSender(), 1_000_000_000n, {
    jettonAmount: 1n,
    destDomain: 99,
    recipient32: unsupportedRecipient32,
  });
  {
    const tx = findTxByAddress(burnUnsupported.transactions, wallet.address);
    assert.ok(tx, 'expected a wallet tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_DOMAIN_UNSUPPORTED);
  }
  const walletAfterUnsupportedBurn = await wallet.getWalletData();
  assert.equal(walletAfterUnsupportedBurn.jettonBalance, walletBeforeUnsupportedBurn.jettonBalance);
  const cfgAfterUnsupportedBurn = await master.getSccpConfig();
  assert.equal(cfgAfterUnsupportedBurn.nonce, cfgBeforeUnsupportedBurn.nonce);
  const unsupportedAttemptMessageId = await master.getOutboundMessageId(
    99,
    cfgBeforeUnsupportedBurn.nonce + 1n,
    1n,
    unsupportedRecipient32,
  );
  const unsupportedAttemptRecord = await master.getBurnRecord(unsupportedAttemptMessageId);
  assert.equal(unsupportedAttemptRecord, null);

  // Disabled local admin controls fail before domain validation.
  const pauseInboundUnsupported = await master.sendSetInboundPaused(
    governor.getSender(),
    1_000_000_000n,
    99,
    true,
  );
  {
    const tx = findTxByAddress(pauseInboundUnsupported.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);
  }

  const pauseOutboundUnsupported = await master.sendSetOutboundPaused(
    governor.getSender(),
    1_000_000_000n,
    99,
    true,
  );
  {
    const tx = findTxByAddress(pauseOutboundUnsupported.transactions, master.address);
    assert.ok(tx, 'expected a master tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_ADMIN_PATH_DISABLED);
  }

  // Burn to an EVM domain with canonical encoding should succeed and persist burn record.
  const canonicalEvmRecipient32 = BigInt(
    '0x' + '00'.repeat(12) + '11'.repeat(20),
  );
  const walletBeforeCanonicalEvmBurn = await wallet.getWalletData();
  const cfgBeforeCanonicalEvmBurn = await master.getSccpConfig();
  const burnCanonicalEvm = await wallet.sendSccpBurnToDomain(
    alice.getSender(),
    1_000_000_000n,
    {
      jettonAmount: 1n,
      destDomain: DOMAIN_ETH,
      recipient32: canonicalEvmRecipient32,
    },
  );
  {
    const tx = findTxByAddress(burnCanonicalEvm.transactions, wallet.address);
    assert.ok(tx, 'expected a wallet tx');
    assert.equal(txExitCode(tx), 0);
  }
  const walletAfterCanonicalEvmBurn = await wallet.getWalletData();
  assert.equal(
    walletAfterCanonicalEvmBurn.jettonBalance,
    walletBeforeCanonicalEvmBurn.jettonBalance - 1n,
  );
  const cfgAfterCanonicalEvmBurn = await master.getSccpConfig();
  assert.equal(
    cfgAfterCanonicalEvmBurn.nonce,
    cfgBeforeCanonicalEvmBurn.nonce + 1n,
  );
  const canonicalEvmMessageId = await master.getOutboundMessageId(
    DOMAIN_ETH,
    cfgAfterCanonicalEvmBurn.nonce,
    1n,
    canonicalEvmRecipient32,
  );
  const canonicalEvmRecord = await master.getBurnRecord(canonicalEvmMessageId);
  assert.ok(canonicalEvmRecord, 'expected canonical EVM burn record to exist');
  const sCanonicalEvm = canonicalEvmRecord.beginParse();
  const canonicalEvmBurnInitiator = sCanonicalEvm.loadAddress();
  const canonicalEvmDestDomain = sCanonicalEvm.loadUint(32);
  const canonicalEvmRecipientStored = sCanonicalEvm.loadUintBig(256);
  const canonicalEvmJettonAmount = sCanonicalEvm.loadCoins();
  const canonicalEvmNonce = sCanonicalEvm.loadUintBig(64);
  assert.equal(
    canonicalEvmBurnInitiator.toRawString(),
    alice.address.toRawString(),
  );
  assert.equal(canonicalEvmDestDomain, DOMAIN_ETH);
  assert.equal(canonicalEvmRecipientStored, canonicalEvmRecipient32);
  assert.equal(canonicalEvmJettonAmount, 1n);
  assert.equal(canonicalEvmNonce, cfgAfterCanonicalEvmBurn.nonce);

  // Burn to non-EVM domain must allow full 256-bit recipient values and persist exactly.
  const oddSoraRecipient32 = BigInt('0x' + '80' + '00'.repeat(31));
  const walletBeforeOddSoraBurn = await wallet.getWalletData();
  const cfgBeforeOddSoraBurn = await master.getSccpConfig();
  const burnOddSora = await wallet.sendSccpBurnToDomain(
    alice.getSender(),
    1_000_000_000n,
    {
      jettonAmount: 1n,
      destDomain: DOMAIN_SORA,
      recipient32: oddSoraRecipient32,
    },
  );
  {
    const tx = findTxByAddress(burnOddSora.transactions, wallet.address);
    assert.ok(tx, 'expected a wallet tx');
    assert.equal(txExitCode(tx), 0);
  }
  const walletAfterOddSoraBurn = await wallet.getWalletData();
  assert.equal(
    walletAfterOddSoraBurn.jettonBalance,
    walletBeforeOddSoraBurn.jettonBalance - 1n,
  );
  const cfgAfterOddSoraBurn = await master.getSccpConfig();
  assert.equal(cfgAfterOddSoraBurn.nonce, cfgBeforeOddSoraBurn.nonce + 1n);
  const oddSoraMessageId = await master.getOutboundMessageId(
    DOMAIN_SORA,
    cfgAfterOddSoraBurn.nonce,
    1n,
    oddSoraRecipient32,
  );
  const oddSoraRecord = await master.getBurnRecord(oddSoraMessageId);
  assert.ok(oddSoraRecord, 'expected non-EVM odd-recipient burn record');
  const sOddSora = oddSoraRecord.beginParse();
  sOddSora.loadAddress(); // initiator already covered above
  const oddSoraDestDomain = sOddSora.loadUint(32);
  const oddSoraRecipientStored = sOddSora.loadUintBig(256);
  assert.equal(oddSoraDestDomain, DOMAIN_SORA);
  assert.equal(oddSoraRecipientStored, oddSoraRecipient32);
});

test('SCCP verifier rejects duplicate validator signer addresses even with valid merkle proofs', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  function keccak256Buf(data) {
    return Buffer.from(ethers.keccak256(data).slice(2), 'hex');
  }

  function keccakPair(a32, b32) {
    return keccak256Buf(Buffer.concat([a32, b32]));
  }

  function buildMerkleRootAndProofs(leaves) {
    let level = leaves.slice();
    const levels = [level];
    while (level.length > 1) {
      const next = [];
      for (let i = 0; i < level.length; i += 2) {
        if (i + 1 >= level.length) {
          next.push(level[i]);
        } else {
          next.push(keccakPair(level[i], level[i + 1]));
        }
      }
      level = next;
      levels.push(level);
    }

    const root = levels[levels.length - 1][0];
    const proofs = leaves.map((_leaf, leafIndex) => {
      const out = [];
      let idx = leafIndex;
      for (let d = 0; d < levels.length - 1; d++) {
        const layer = levels[d];
        const sib = idx % 2 === 1 ? idx - 1 : idx + 1;
        if (sib < layer.length) {
          out.push(layer[sib]);
        }
        idx = Math.floor(idx / 2);
      }
      return out;
    });
    return { root, proofs };
  }

  // Intentionally duplicate validator key at positions 0 and 1.
  const validatorPrivKeys = [
    '0x' + '11'.repeat(32),
    '0x' + '11'.repeat(32),
    '0x' + '22'.repeat(32),
    '0x' + '33'.repeat(32),
  ];
  const validators = validatorPrivKeys.map((pk) => {
    const w = new ethers.Wallet(pk);
    const addr20 = Buffer.from(w.address.slice(2).padStart(40, '0'), 'hex');
    return { pk, addr20 };
  });

  const leaves = validators.map((v) => keccak256Buf(v.addr20));
  const { root: validatorSetRoot32, proofs: validatorProofs32 } = buildMerkleRootAndProofs(leaves);
  const validatorSetRootU256 = BigInt('0x' + validatorSetRoot32.toString('hex'));

  const currentValidatorSetId = 1n;
  const nextValidatorSetId = 2n;
  const validatorSetLen = 4;

  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId,
    currentValidatorSetLen: validatorSetLen,
    currentValidatorSetRootU256: validatorSetRootU256,
    nextValidatorSetId,
    nextValidatorSetLen: validatorSetLen,
    nextValidatorSetRootU256: validatorSetRootU256,
  });

  const digestScale = Buffer.concat([
    Buffer.from([0x04, 0x00, 0x02, 0x50, 0x43, 0x43, 0x53]),
    Buffer.alloc(32, 0xAB),
  ]);
  const digestHash32 = Buffer.from(ethers.keccak256(digestScale).slice(2), 'hex');
  const proofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: digestScale,
    nextAuthoritySetId: nextValidatorSetId,
    nextAuthoritySetLen: validatorSetLen,
    nextAuthoritySetRootU256: validatorSetRootU256,
  });

  // Rebuild the leaf SCALE bytes to compute commitment.mmr_root exactly as verifier expects.
  const leafScale = Buffer.alloc(145);
  leafScale[0] = 0;
  leafScale.writeUInt32LE(0, 1);
  leafScale.writeBigUInt64LE(nextValidatorSetId, 37);
  leafScale.writeUInt32LE(validatorSetLen, 45);
  Buffer.from(validatorSetRootU256.toString(16).padStart(64, '0'), 'hex').copy(leafScale, 49);
  digestHash32.copy(leafScale, 113);
  const mmrRoot32 = keccak256Buf(leafScale);
  const commitmentMmrRootU256 = BigInt('0x' + mmrRoot32.toString('hex'));

  const commitmentBlockNumber = 10;
  const commitmentScale = Buffer.alloc(48);
  commitmentScale[0] = 0x04;
  commitmentScale[1] = 'm'.charCodeAt(0);
  commitmentScale[2] = 'h'.charCodeAt(0);
  commitmentScale[3] = 0x80;
  mmrRoot32.copy(commitmentScale, 4);
  commitmentScale.writeUInt32LE(commitmentBlockNumber, 36);
  commitmentScale.writeBigUInt64LE(currentValidatorSetId, 40);
  const commitmentHashHex = ethers.keccak256(commitmentScale);

  function buildMerkleProofCell(siblings32) {
    const b = beginCell();
    b.storeUint(siblings32.length, 16);
    for (const sib of siblings32) {
      b.storeUint(BigInt('0x' + sib.toString('hex')), 256);
    }
    return b.endCell();
  }

  function buildValidatorProofCell(entries) {
    let next = null;
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i];
      const merkleProofCell = e.merkleProofCell ?? buildMerkleProofCell(e.merkleProofSiblings32);
      next = beginCell()
        .storeUint(e.v, 8)
        .storeUint(e.r, 256)
        .storeUint(e.s, 256)
        .storeUint(e.pos, 32)
        .storeRef(merkleProofCell)
        .storeMaybeRef(next)
        .endCell();
    }
    return beginCell()
      .storeUint(entries.length, 16)
      .storeRef(next)
      .endCell();
  }

  // Positions are unique and proofs are valid, but positions 0 and 1 are signed by the same validator.
  const signingEntries = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCell = buildValidatorProofCell(signingEntries);

  const submitOut = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitOut.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_PROOF);
  }
});

test('SCCP verifier rejects malleable high-s signatures', async () => {
  const verifierArtifact = loadArtifact('sccp-sora-verifier.compiled.json');
  const verifierCode = codeFromArtifact(verifierArtifact);

  const blockchain = await Blockchain.create();
  const governor = await blockchain.treasury('governor');
  const alice = await blockchain.treasury('alice');
  const dummyMaster = await blockchain.treasury('dummy_master');

  const soraAssetIdU256 = BigInt('0x' + '11'.repeat(32));
  const verifier = blockchain.openContract(
    SccpSoraVerifier.createFromArtifacts(verifierCode, governor.address, dummyMaster.address, soraAssetIdU256),
  );
  await verifier.sendDeploy(governor.getSender(), 1_000_000_000n);

  function keccak256Buf(data) {
    return Buffer.from(ethers.keccak256(data).slice(2), 'hex');
  }

  function keccakPair(a32, b32) {
    return keccak256Buf(Buffer.concat([a32, b32]));
  }

  function buildMerkleRootAndProofs(leaves) {
    let level = leaves.slice();
    const levels = [level];
    while (level.length > 1) {
      const next = [];
      for (let i = 0; i < level.length; i += 2) {
        if (i + 1 >= level.length) {
          next.push(level[i]);
        } else {
          next.push(keccakPair(level[i], level[i + 1]));
        }
      }
      level = next;
      levels.push(level);
    }

    const root = levels[levels.length - 1][0];
    const proofs = leaves.map((_leaf, leafIndex) => {
      const out = [];
      let idx = leafIndex;
      for (let d = 0; d < levels.length - 1; d++) {
        const layer = levels[d];
        const sib = idx % 2 === 1 ? idx - 1 : idx + 1;
        if (sib < layer.length) {
          out.push(layer[sib]);
        }
        idx = Math.floor(idx / 2);
      }
      return out;
    });
    return { root, proofs };
  }

  const validatorPrivKeys = [
    '0x' + '11'.repeat(32),
    '0x' + '22'.repeat(32),
    '0x' + '33'.repeat(32),
    '0x' + '44'.repeat(32),
  ];
  const validators = validatorPrivKeys.map((pk) => {
    const w = new ethers.Wallet(pk);
    const addr20 = Buffer.from(w.address.slice(2).padStart(40, '0'), 'hex');
    return { pk, addr20 };
  });

  const leaves = validators.map((v) => keccak256Buf(v.addr20));
  const { root: validatorSetRoot32, proofs: validatorProofs32 } = buildMerkleRootAndProofs(leaves);
  const validatorSetRootU256 = BigInt('0x' + validatorSetRoot32.toString('hex'));

  const currentValidatorSetId = 1n;
  const nextValidatorSetId = 2n;
  const validatorSetLen = 4;

  await verifier.sendInitialize(governor.getSender(), 1_000_000_000n, {
    latestBeefyBlock: 0,
    currentValidatorSetId,
    currentValidatorSetLen: validatorSetLen,
    currentValidatorSetRootU256: validatorSetRootU256,
    nextValidatorSetId,
    nextValidatorSetLen: validatorSetLen,
    nextValidatorSetRootU256: validatorSetRootU256,
  });

  const digestScale = Buffer.concat([
    Buffer.from([0x04, 0x00, 0x02, 0x50, 0x43, 0x43, 0x53]),
    Buffer.alloc(32, 0xAB),
  ]);
  const digestHash32 = Buffer.from(ethers.keccak256(digestScale).slice(2), 'hex');
  const proofCell = buildSoraLeafProofWithDigest({
    digestScaleBytes: digestScale,
    nextAuthoritySetId: nextValidatorSetId,
    nextAuthoritySetLen: validatorSetLen,
    nextAuthoritySetRootU256: validatorSetRootU256,
  });

  const leafScale = Buffer.alloc(145);
  leafScale[0] = 0;
  leafScale.writeUInt32LE(0, 1);
  leafScale.writeBigUInt64LE(nextValidatorSetId, 37);
  leafScale.writeUInt32LE(validatorSetLen, 45);
  Buffer.from(validatorSetRootU256.toString(16).padStart(64, '0'), 'hex').copy(leafScale, 49);
  digestHash32.copy(leafScale, 113);
  const mmrRoot32 = keccak256Buf(leafScale);
  const commitmentMmrRootU256 = BigInt('0x' + mmrRoot32.toString('hex'));

  const commitmentBlockNumber = 10;
  const commitmentScale = Buffer.alloc(48);
  commitmentScale[0] = 0x04;
  commitmentScale[1] = 'm'.charCodeAt(0);
  commitmentScale[2] = 'h'.charCodeAt(0);
  commitmentScale[3] = 0x80;
  mmrRoot32.copy(commitmentScale, 4);
  commitmentScale.writeUInt32LE(commitmentBlockNumber, 36);
  commitmentScale.writeBigUInt64LE(currentValidatorSetId, 40);
  const commitmentHashHex = ethers.keccak256(commitmentScale);

  function buildMerkleProofCell(siblings32) {
    const b = beginCell();
    b.storeUint(siblings32.length, 16);
    for (const sib of siblings32) {
      b.storeUint(BigInt('0x' + sib.toString('hex')), 256);
    }
    return b.endCell();
  }

  function buildValidatorProofCell(entries) {
    let next = null;
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i];
      const merkleProofCell = e.merkleProofCell ?? buildMerkleProofCell(e.merkleProofSiblings32);
      next = beginCell()
        .storeUint(e.v, 8)
        .storeUint(e.r, 256)
        .storeUint(e.s, 256)
        .storeUint(e.pos, 32)
        .storeRef(merkleProofCell)
        .storeMaybeRef(next)
        .endCell();
    }
    return beginCell()
      .storeUint(entries.length, 16)
      .storeRef(next)
      .endCell();
  }

  const signingEntries = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      // Flip to malleable high-s form; verifier must fail-closed.
      s: idx === 0 ? (SECP256K1N - BigInt(sig.s)) : BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCell = buildValidatorProofCell(signingEntries);

  const submitOut = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitOut.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_SIGNATURE);
  }

  // Zero-r signatures must fail-closed.
  const signingEntriesZeroR = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: idx === 0 ? 0n : BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCellZeroR = buildValidatorProofCell(signingEntriesZeroR);
  const submitZeroR = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellZeroR,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitZeroR.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_SIGNATURE);
  }

  // Zero-s signatures must fail-closed.
  const signingEntriesZeroS = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: idx === 0 ? 0n : BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCellZeroS = buildValidatorProofCell(signingEntriesZeroS);
  const submitZeroS = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellZeroS,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitZeroS.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_SIGNATURE);
  }

  // Invalid recovery ids must fail-closed (both raw and 27/28-offset forms).
  for (const invalidV of [2, 3, 29, 30, 31, 32, 255]) {
    const signingEntriesBadV = [0, 1, 2].map((idx) => {
      const v = validators[idx];
      const sk = new ethers.SigningKey(v.pk);
      const sig = sk.sign(commitmentHashHex);
      return {
        v: idx === 0 ? invalidV : (27 + sig.yParity),
        r: BigInt(sig.r),
        s: BigInt(sig.s),
        pos: idx,
        merkleProofSiblings32: validatorProofs32[idx],
      };
    });
    const validatorProofCellBadV = buildValidatorProofCell(signingEntriesBadV);
    const submitBadV = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
      commitmentMmrRootU256,
      commitmentBlockNumber,
      commitmentValidatorSetId: currentValidatorSetId,
      validatorProofCell: validatorProofCellBadV,
      latestLeafProofCell: proofCell,
    });
    const tx = findTxByAddress(submitBadV.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_SIGNATURE);
  }

  // Duplicate validator positions must fail-closed.
  const signingEntriesDuplicatePos = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx === 1 ? 0 : idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCellDuplicatePos = buildValidatorProofCell(signingEntriesDuplicatePos);
  const submitDuplicatePos = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellDuplicatePos,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitDuplicatePos.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_PROOF);
  }

  // Non-increasing validator positions must fail-closed.
  const signingEntriesUnsortedPos = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx === 0 ? 1 : (idx === 1 ? 0 : 2),
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCellUnsortedPos = buildValidatorProofCell(signingEntriesUnsortedPos);
  const submitUnsortedPos = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellUnsortedPos,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitUnsortedPos.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_PROOF);
  }

  // Out-of-range validator positions must fail-closed.
  const signingEntriesOutOfRangePos = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx === 2 ? validatorSetLen : idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCellOutOfRangePos = buildValidatorProofCell(signingEntriesOutOfRangePos);
  const submitOutOfRangePos = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellOutOfRangePos,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitOutOfRangePos.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_PROOF);
  }

  // Valid path with extra trailing Merkle sibling must fail-closed.
  const signingEntriesExtraMerkleSibling = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: idx === 0
        ? [...validatorProofs32[idx], Buffer.alloc(32)]
        : validatorProofs32[idx],
    };
  });
  const validatorProofCellExtraMerkleSibling = buildValidatorProofCell(signingEntriesExtraMerkleSibling);
  const submitExtraMerkleSibling = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellExtraMerkleSibling,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitExtraMerkleSibling.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_PROOF);
  }

  // Valid sibling count with an extra trailing Merkle ref must also fail-closed.
  const trailingMerkleRefBuilder = beginCell()
    .storeUint(validatorProofs32[0].length, 16);
  for (const sib of validatorProofs32[0]) {
    trailingMerkleRefBuilder.storeUint(BigInt('0x' + sib.toString('hex')), 256);
  }
  trailingMerkleRefBuilder.storeRef(beginCell().storeUint(0, 8).endCell());
  const trailingMerkleRefCell = trailingMerkleRefBuilder.endCell();
  const signingEntriesTrailingMerkleRef = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
      merkleProofCell: idx === 0 ? trailingMerkleRefCell : undefined,
    };
  });
  const validatorProofCellTrailingMerkleRef = buildValidatorProofCell(signingEntriesTrailingMerkleRef);
  const submitTrailingMerkleRef = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellTrailingMerkleRef,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitTrailingMerkleRef.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_INVALID_VALIDATOR_PROOF);
  }

  // Low-v parity signatures (0/1) should be accepted.
  const signingEntriesLowV = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHex);
    return {
      v: sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCellLowV = buildValidatorProofCell(signingEntriesLowV);
  const submitLowV = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellLowV,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitLowV.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), 0);
  }

  // Canonical high-v signatures (27/28) should also be accepted.
  const commitmentBlockNumberHighV = commitmentBlockNumber + 1;
  const commitmentScaleHighV = Buffer.alloc(48);
  commitmentScaleHighV[0] = 0x04;
  commitmentScaleHighV[1] = 'm'.charCodeAt(0);
  commitmentScaleHighV[2] = 'h'.charCodeAt(0);
  commitmentScaleHighV[3] = 0x80;
  mmrRoot32.copy(commitmentScaleHighV, 4);
  commitmentScaleHighV.writeUInt32LE(commitmentBlockNumberHighV, 36);
  commitmentScaleHighV.writeBigUInt64LE(currentValidatorSetId, 40);
  const commitmentHashHexHighV = ethers.keccak256(commitmentScaleHighV);

  const signingEntriesHighV = [0, 1, 2].map((idx) => {
    const v = validators[idx];
    const sk = new ethers.SigningKey(v.pk);
    const sig = sk.sign(commitmentHashHexHighV);
    return {
      v: 27 + sig.yParity,
      r: BigInt(sig.r),
      s: BigInt(sig.s),
      pos: idx,
      merkleProofSiblings32: validatorProofs32[idx],
    };
  });
  const validatorProofCellHighV = buildValidatorProofCell(signingEntriesHighV);
  const submitHighV = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber: commitmentBlockNumberHighV,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellHighV,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitHighV.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), 0);
  }

  // Replay of the same commitment must fail once latest block is updated.
  const submitLowVReplay = await verifier.sendSubmitSignatureCommitment(alice.getSender(), 1_000_000_000n, {
    commitmentMmrRootU256,
    commitmentBlockNumber,
    commitmentValidatorSetId: currentValidatorSetId,
    validatorProofCell: validatorProofCellLowV,
    latestLeafProofCell: proofCell,
  });
  {
    const tx = findTxByAddress(submitLowVReplay.transactions, verifier.address);
    assert.ok(tx, 'expected a verifier tx');
    assert.equal(txExitCode(tx), ERROR_SCCP_COMMITMENT_TOO_OLD);
  }
});
