// SPDX-License-Identifier: BSD-4-Clause
//
// Jetton + SCCP message definitions.

import "storage"

type ForwardPayloadRemainder = RemainingBitsAndRefs

// --- Jetton standard-ish messages (TEP-74 compatible surface) ---

struct (0x0f8a7ea5) AskToTransfer {
    queryId: uint64
    jettonAmount: coins
    transferRecipient: address
    sendExcessesTo: address?
    customPayload: cell?
    forwardTonAmount: coins
    forwardPayload: ForwardPayloadRemainder
}

struct (0x7362d09c) TransferNotificationForRecipient {
    queryId: uint64
    jettonAmount: coins
    transferInitiator: address?
    forwardPayload: ForwardPayloadRemainder
}

struct (0x178d4519) InternalTransferStep {
    queryId: uint64
    jettonAmount: coins
    transferInitiator: address?
    sendExcessesTo: address?
    forwardTonAmount: coins
    forwardPayload: ForwardPayloadRemainder
}

struct (0xd53276db) ReturnExcessesBack {
    queryId: uint64
}

struct (0x595f07bc) AskToBurn {
    queryId: uint64
    jettonAmount: coins
    sendExcessesTo: address?
    customPayload: cell?
}

struct (0x7bdd97de) BurnNotificationForMinter {
    queryId: uint64
    jettonAmount: coins
    burnInitiator: address
    sendExcessesTo: address?
}

struct (0x2c76b973) RequestWalletAddress {
    queryId: uint64
    ownerAddress: address
    includeOwnerAddress: bool
}

struct (0xd1735400) ResponseWalletAddress {
    queryId: uint64
    jettonWalletAddress: address?
    ownerAddress: Cell<address>?
}

struct (0xd372158c) TopUpTons {
}

// --- SCCP extensions ---

// Owner -> Jetton wallet: burn jettons targeting a destination SCCP domain and recipient32.
struct (0x4f80d7e1) SccpBurnToDomain {
    queryId: uint64
    jettonAmount: coins
    destDomain: uint32
    recipient32: uint256
    sendExcessesTo: address?
}

struct SccpBurnDetails {
    destDomain: uint32
    recipient32: uint256
}

// Jetton wallet -> Jetton master: SCCP burn notification that the master stores as a burn record.
struct (0x6b2e76a4) SccpBurnNotificationForMinter {
    queryId: uint64
    jettonAmount: coins
    burnInitiator: address
    details: Cell<SccpBurnDetails>
    sendExcessesTo: address?
}

// Jetton master -> burn initiator: lightweight notification containing messageId and nonce.
struct (0x1fd0ab62) SccpBurnedNotification {
    queryId: uint64
    messageId: uint256
    nonce: uint64
}

// Verifier -> Jetton master: mint jettons for a verified SCCP burn on another chain.
// Fail-closed until `verifierAddress` is self-registered once by the verifier contract.
struct (0x23e4c1a0) SccpMintFromVerifier {
    queryId: uint64
    sourceDomain: uint32
    burnNonce: uint64
    jettonAmount: coins
    recipient32: uint256
    sendExcessesTo: address?
}

// Deprecated local admin operations. Contracts keep decoding these opcodes for compatibility,
// but runtime handlers fail closed with `ERROR_SCCP_ADMIN_PATH_DISABLED`.
struct (0x55a3f6d1) SccpSetGovernor {
    queryId: uint64
    newGovernor: address
}

struct (0x0f95e281) SccpSetVerifier {
    queryId: uint64
    newVerifier: address?
}

struct (0x3bf64dc2) SccpSetInboundPaused {
    queryId: uint64
    sourceDomain: uint32
    paused: bool
}

struct (0x91f4c2a7) SccpSetOutboundPaused {
    queryId: uint64
    destDomain: uint32
    paused: bool
}

struct (0x4a28c9d7) SccpInvalidateInboundMessage {
    queryId: uint64
    messageId: uint256
}

struct (0x6c1e27b4) SccpRevalidateInboundMessage {
    queryId: uint64
    messageId: uint256
}

struct (0x6d4bbf4a) SccpSetMetadataUri {
    queryId: uint64
    newMetadataUri: SnakeString
}

// --- SCCP verifier (SORA -> TON) ---

// Deprecated local admin operation kept only for opcode compatibility.
struct (0x9a6d3b21) SccpVerifierSetGovernor {
    queryId: uint64
    newGovernor: address
}

// Anyone -> Verifier: one-time light client bootstrap.
//
// The initial validator sets and `latestBeefyBlock` must be sourced from SORA chain state.
struct (0x35f2bca1) SccpVerifierInitialize {
    queryId: uint64
    latestBeefyBlock: uint64
    currentValidatorSetId: uint64
    currentValidatorSetLen: uint32
    currentValidatorSetRoot: uint256
    nextValidatorSetId: uint64
    nextValidatorSetLen: uint32
    nextValidatorSetRoot: uint256
}

// Anyone -> Verifier: import a new finalized SORA MMR root by verifying a BEEFY commitment.
//
// This is the TON analogue of the EVM/Solana light client flow:
// - verify >=2/3 validator signatures over the commitment hash (secp256k1 ECDSA via TVM `ECRECOVER`)
// - verify the provided leaf+MMR proof yields `commitmentMmrRoot`
// - add the MMR root to a fixed-size on-chain ring buffer for later burn proof verification
struct (0x6a4df0b3) SccpVerifierSubmitSignatureCommitment {
    queryId: uint64
    commitmentMmrRoot: uint256
    commitmentBlockNumber: uint32
    commitmentValidatorSetId: uint64
    validatorProof: cell
    latestLeafProof: cell
}

// Anyone -> Verifier: verify a SORA burn proof and mint via `SccpMintFromVerifier` to the Jetton master.
struct (0x1a9b2c7d) SccpVerifierMintFromSoraProof {
    queryId: uint64
    burnNonce: uint64
    jettonAmount: coins
    recipient32: uint256
    sendExcessesTo: address?
    proof: cell
}

// Anyone -> Verifier: verify a SORA burn proof and mint via `SccpMintFromVerifier` to the Jetton master,
// but with an explicit `sourceDomain`.
//
// This enables minting on TON from SCCP burns on any source chain, as long as SORA committed that burn's
// `messageId` into its auxiliary digest (BEEFY+MMR finalized).
struct (0x1a9b2c7e) SccpVerifierMintFromSoraProofV2 {
    queryId: uint64
    sourceDomain: uint32
    burnNonce: uint64
    jettonAmount: coins
    recipient32: uint256
    sendExcessesTo: address?
    proof: cell
}

// "forward payload" is TL/B `(Either Cell ^Cell)`;
// if it's ^Cell, no other data should exist in a slice.
fun ForwardPayloadRemainder.checkIsCorrectTLBEither(self) {
    var mutableCopy = self;
    if (mutableCopy.loadMaybeRef() != null) {
        mutableCopy.assertEnd()
    }
}
