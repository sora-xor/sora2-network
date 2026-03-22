// SPDX-License-Identifier: BSD-4-Clause
//
// SCCP-enabled Jetton master (minter) contract.
//
// Security posture:
// - Inbound minting is fail-closed until `verifierAddress` is set.
// - `verifierAddress` can be pinned exactly once by the configured governor.
// - All other local admin mutation paths are disabled.
//
// Based on `ton-blockchain/tolk-bench` Jetton minter (MIT), adapted for SCCP.

import "@stdlib/gas-payments"
import "errors"
import "jetton-utils"
import "storage"
import "messages"
import "sccp-codec"

type AllowedMessageToMinter =
    | BurnNotificationForMinter
    | SccpBurnNotificationForMinter
    | RequestWalletAddress
    | SccpMintFromVerifier
    | SccpAddTokenFromVerifier
    | SccpPauseTokenFromVerifier
    | SccpResumeTokenFromVerifier
    | SccpSetGovernor
    | SccpSetVerifier
    | SccpSetInboundPaused
    | SccpSetOutboundPaused
    | SccpInvalidateInboundMessage
    | SccpRevalidateInboundMessage
    | SccpSetMetadataUri
    | TopUpTons

// The master receives bounces only for messages it sent (mint internal transfers).
fun onBouncedMessage(in: InMessageBounced) {
    // We use rich bounces for SCCP mints to reliably clear processed markers.
    val rich = lazy RichBounceBody.fromSlice(in.bouncedBody);
    val msg = lazy InternalTransferStep.fromSlice(rich.originalBody.beginParse());

    // Restore supply (mint transfer failed).
    var storage = lazy MinterStorage.load();
    storage.totalSupply -= msg.jettonAmount;

    // If the forwardPayload contains SCCP messageId, clear processed marker so mint can be retried.
    var sccp = lazy storage.sccp.load();
    var fp = msg.forwardPayload;
    val maybeRef = fp.loadMaybeRef();
    if (maybeRef != null) {
        var refSlice = maybeRef!.beginParse();
        val messageId = refSlice.loadUint(256);
        sccp.processedInbound.delete(messageId);
    }

    storage.sccp = sccp.toCell();
    storage.save();
}

fun sccpAssertVerifier(storage: MinterStorage, sender: address) {
    assert (storage.verifierAddress != null) throw ERROR_SCCP_VERIFIER_NOT_SET;
    assert (sender == storage.verifierAddress!) throw ERROR_SCCP_NOT_VERIFIER;
}

fun sccpAssertGovernanceMessageFresh(mutate sccp: SccpStorageExtra, messageId: uint256) {
    assert (!sccp.processedGovernance.get(messageId).isFound) throw ERROR_SCCP_MESSAGE_ALREADY_PROCESSED;
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessageToMinter.fromSlice(in.body);

    match (msg) {
        BurnNotificationForMinter => {
            var storage = lazy MinterStorage.load();
            assert (in.senderAddress == calcAddressOfJettonWallet(msg.burnInitiator, contract.getAddress(), storage.jettonWalletCode)) throw ERROR_NOT_VALID_WALLET;
            storage.totalSupply -= msg.jettonAmount;
            storage.save();

            if (msg.sendExcessesTo == null) {
                return;
            }

            val excessesMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: msg.sendExcessesTo,
                value: 0,
                body: ReturnExcessesBack {
                    queryId: msg.queryId
                }
            });
            excessesMsg.send(SEND_MODE_IGNORE_ERRORS | SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
        }

        SccpBurnNotificationForMinter => {
            var storage = lazy MinterStorage.load();
            var sccp = lazy storage.sccp.load();
            assert (in.senderAddress == calcAddressOfJettonWallet(msg.burnInitiator, contract.getAddress(), storage.jettonWalletCode)) throw ERROR_NOT_VALID_WALLET;
            storage.totalSupply -= msg.jettonAmount;

            val details = lazy msg.details.load();
            sccpEnsureSupportedDomain(details.destDomain);
            assert (details.destDomain != SCCP_DOMAIN_TON) throw ERROR_SCCP_DOMAIN_UNSUPPORTED;
            assert (!sccpIsRecipientZero(details.recipient32)) throw ERROR_SCCP_RECIPIENT_IS_ZERO;
            if (sccpIsEvmDomain(details.destDomain)) {
                assert (sccpIsCanonicalEvmRecipient(details.recipient32)) throw ERROR_SCCP_RECIPIENT_NOT_CANONICAL;
            }
            assert (!sccpMaskHasDomain(sccp.outboundPausedMask, details.destDomain)) throw ERROR_SCCP_OUTBOUND_PAUSED;
            assert (sccp.tokenState == SCCP_TOKEN_STATE_ACTIVE) throw ERROR_SCCP_TOKEN_NOT_ACTIVE;

            assert (sccp.nonce < MAX_U64) throw ERROR_SCCP_NONCE_OVERFLOW;
            sccp.nonce += 1;

            val messageId = sccpBurnMessageIdV1(
                SCCP_DOMAIN_TON,
                details.destDomain,
                sccp.nonce,
                sccp.soraAssetId,
                msg.jettonAmount,
                details.recipient32,
            );

            // Store burn record for on-chain proof.
            sccp.burns.set(
                messageId,
                SccpBurnRecord {
                    burnInitiator: msg.burnInitiator,
                    destDomain: details.destDomain,
                    recipient32: details.recipient32,
                    jettonAmount: msg.jettonAmount,
                    nonce: sccp.nonce,
                }.toCell()
            );

            storage.sccp = sccp.toCell();
            storage.save();

            // Best-effort notify initiator about message id (helps off-chain UX).
            val notifyMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: msg.burnInitiator,
                value: 0,
                body: SccpBurnedNotification {
                    queryId: msg.queryId,
                    messageId,
                    nonce: sccp.nonce,
                }
            });
            notifyMsg.send(SEND_MODE_IGNORE_ERRORS | SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);

            if (msg.sendExcessesTo != null) {
                val excessesMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: msg.sendExcessesTo,
                    value: 0,
                    body: ReturnExcessesBack {
                        queryId: msg.queryId
                    }
                });
                excessesMsg.send(SEND_MODE_IGNORE_ERRORS | SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
            }
        }

        RequestWalletAddress => {
            var ownerAddress: Cell<address>? = msg.includeOwnerAddress
                ? msg.ownerAddress.toCell()
                : null;

            var walletAddress: address? = null;
            if (msg.ownerAddress.getWorkchain() == MY_WORKCHAIN) {
                val storage = lazy MinterStorage.load();
                walletAddress = calcAddressOfJettonWallet(msg.ownerAddress, contract.getAddress(), storage.jettonWalletCode);
            }

            val respondMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: in.senderAddress,
                value: 0,
                body: ResponseWalletAddress {
                    queryId: msg.queryId,
                    jettonWalletAddress: walletAddress,
                    ownerAddress: ownerAddress,
                }
            });
            respondMsg.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
        }

        SccpMintFromVerifier => {
            var storage = lazy MinterStorage.load();
            var sccp = lazy storage.sccp.load();

            sccpAssertVerifier(storage, in.senderAddress);
            sccpEnsureSupportedDomain(msg.sourceDomain);
            assert (msg.sourceDomain != SCCP_DOMAIN_TON) throw ERROR_SCCP_DOMAIN_UNSUPPORTED;

            // Incident controls.
            assert (!sccpMaskHasDomain(sccp.inboundPausedMask, msg.sourceDomain)) throw ERROR_SCCP_INBOUND_PAUSED;

            assert (!sccpIsRecipientZero(msg.recipient32)) throw ERROR_SCCP_RECIPIENT_IS_ZERO;
            assert (sccp.tokenState == SCCP_TOKEN_STATE_ACTIVE) throw ERROR_SCCP_TOKEN_NOT_ACTIVE;

            // Compute canonical SCCP messageId to gate replay/invalidation.
            val messageId = sccpBurnMessageIdV1(
                msg.sourceDomain,
                SCCP_DOMAIN_TON,
                msg.burnNonce,
                sccp.soraAssetId,
                msg.jettonAmount,
                msg.recipient32,
            );

            assert (!sccp.invalidatedInbound.get(messageId).isFound) throw ERROR_SCCP_MESSAGE_INVALIDATED;
            assert (!sccp.processedInbound.get(messageId).isFound) throw ERROR_SCCP_MESSAGE_ALREADY_PROCESSED;

            // Mark processed before minting; bounce handler will clear marker if mint fails.
            sccp.processedInbound.set(messageId, true);

            // Update supply, then mint to recipient.
            storage.totalSupply += msg.jettonAmount;
            storage.sccp = sccp.toCell();
            storage.save();

            reserveToncoinsOnBalance(ton("0.02"), RESERVE_MODE_EXACT_AMOUNT);

            val recipientAddress = address.fromWorkchainAndHash(MY_WORKCHAIN, msg.recipient32);
            assert (recipientAddress.getWorkchain() == MY_WORKCHAIN) throw ERROR_WRONG_WORKCHAIN;

            // Encode messageId into forwardPayload (as a single ref) to support bounce recovery.
            val messageIdRef = beginCell().storeUint(messageId, 256).endCell();
            val forwardPayload = beginCell().storeRef(messageIdRef).endCell().beginParse();

            val deployMsg = createMessage({
                bounce: BounceMode.RichBounce,
                dest: calcDeployedJettonWallet(recipientAddress, contract.getAddress(), storage.jettonWalletCode),
                value: 0,
                body: InternalTransferStep {
                    queryId: msg.queryId,
                    jettonAmount: msg.jettonAmount,
                    transferInitiator: null,
                    sendExcessesTo: msg.sendExcessesTo,
                    forwardTonAmount: 0,
                    forwardPayload,
                }
            });
            deployMsg.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
        }

        SccpAddTokenFromVerifier => {
            var storage = lazy MinterStorage.load();
            var sccp = lazy storage.sccp.load();

            sccpAssertVerifier(storage, in.senderAddress);
            assert (msg.soraAssetId == sccp.soraAssetId) throw ERROR_INVALID_MESSAGE;
            sccpAssertGovernanceMessageFresh(mutate sccp, msg.messageId);
            assert (sccp.tokenState == SCCP_TOKEN_STATE_PAUSED) throw ERROR_SCCP_TOKEN_NOT_PAUSED;

            sccp.processedGovernance.set(msg.messageId, true);
            sccp.tokenState = SCCP_TOKEN_STATE_ACTIVE;
            storage.sccp = sccp.toCell();
            storage.save();
        }

        SccpPauseTokenFromVerifier => {
            var storage = lazy MinterStorage.load();
            var sccp = lazy storage.sccp.load();

            sccpAssertVerifier(storage, in.senderAddress);
            assert (msg.soraAssetId == sccp.soraAssetId) throw ERROR_INVALID_MESSAGE;
            sccpAssertGovernanceMessageFresh(mutate sccp, msg.messageId);
            assert (sccp.tokenState == SCCP_TOKEN_STATE_ACTIVE) throw ERROR_SCCP_TOKEN_NOT_ACTIVE;

            sccp.processedGovernance.set(msg.messageId, true);
            sccp.tokenState = SCCP_TOKEN_STATE_PAUSED;
            storage.sccp = sccp.toCell();
            storage.save();
        }

        SccpResumeTokenFromVerifier => {
            var storage = lazy MinterStorage.load();
            var sccp = lazy storage.sccp.load();

            sccpAssertVerifier(storage, in.senderAddress);
            assert (msg.soraAssetId == sccp.soraAssetId) throw ERROR_INVALID_MESSAGE;
            sccpAssertGovernanceMessageFresh(mutate sccp, msg.messageId);
            assert (sccp.tokenState == SCCP_TOKEN_STATE_PAUSED) throw ERROR_SCCP_TOKEN_NOT_PAUSED;

            sccp.processedGovernance.set(msg.messageId, true);
            sccp.tokenState = SCCP_TOKEN_STATE_ACTIVE;
            storage.sccp = sccp.toCell();
            storage.save();
        }

        SccpSetGovernor => {
            throw ERROR_SCCP_ADMIN_PATH_DISABLED;
        }

        SccpSetVerifier => {
            var storage = lazy MinterStorage.load();
            if (
                storage.verifierAddress == null &&
                msg.newVerifier != null &&
                in.senderAddress == storage.governorAddress
            ) {
                storage.verifierAddress = msg.newVerifier;
                storage.save();
            } else {
                throw ERROR_SCCP_ADMIN_PATH_DISABLED;
            }
        }

        SccpSetInboundPaused => {
            throw ERROR_SCCP_ADMIN_PATH_DISABLED;
        }

        SccpSetOutboundPaused => {
            throw ERROR_SCCP_ADMIN_PATH_DISABLED;
        }

        SccpInvalidateInboundMessage => {
            throw ERROR_SCCP_ADMIN_PATH_DISABLED;
        }

        SccpRevalidateInboundMessage => {
            throw ERROR_SCCP_ADMIN_PATH_DISABLED;
        }

        SccpSetMetadataUri => {
            throw ERROR_SCCP_ADMIN_PATH_DISABLED;
        }

        TopUpTons => {
            // just accept tons
        }

        else => throw 0xFFFF
    }
}

// --- Getters ---

struct JettonDataReply {
    totalSupply: int
    mintable: bool
    adminAddress: address?
    jettonContent: Cell<OnchainMetadataReply>
    jettonWalletCode: cell
}

struct (0x00) OnchainMetadataReply {
    contentDict: map<uint256, Cell<SnakeDataReply>>
}

struct (0x00) SnakeDataReply {
    string: SnakeString
}

get fun get_jetton_data(): JettonDataReply {
    val storage = lazy MinterStorage.load();
    var metadata: OnchainMetadataReply = {
        contentDict: createEmptyMap()
    };
    metadata.contentDict.set(stringSha256("uri"), SnakeDataReply{string: storage.metadataUri.load()}.toCell());
    metadata.contentDict.set(stringSha256("decimals"), SnakeDataReply{string: "9"}.toCell());

    return {
        totalSupply: storage.totalSupply,
        mintable: true,
        adminAddress: storage.governorAddress,
        jettonContent: metadata.toCell(),
        jettonWalletCode: storage.jettonWalletCode,
    }
}

get fun get_wallet_address(ownerAddress: address): address {
    val storage = lazy MinterStorage.load();
    return calcAddressOfJettonWallet(ownerAddress, contract.getAddress(), storage.jettonWalletCode);
}

get fun get_sccp_config(): (address, address?, uint256, uint64, uint64, uint64) {
    val storage = lazy MinterStorage.load();
    val sccp = lazy storage.sccp.load();
    return (storage.governorAddress, storage.verifierAddress, sccp.soraAssetId, sccp.nonce, sccp.inboundPausedMask, sccp.outboundPausedMask);
}

get fun get_sccp_token_state(): int {
    val storage = lazy MinterStorage.load();
    val sccp = lazy storage.sccp.load();
    return sccp.tokenState;
}

get fun get_sccp_burn_record(messageId: uint256): Cell<SccpBurnRecord>? {
    val storage = lazy MinterStorage.load();
    val sccp = lazy storage.sccp.load();
    val r = sccp.burns.get(messageId);
    return r.isFound ? r.loadValue() : null;
}

get fun get_sccp_message_id(destDomain: uint32, nonce: uint64, jettonAmount: coins, recipient32: uint256): uint256 {
    val storage = lazy MinterStorage.load();
    val sccp = lazy storage.sccp.load();
    return sccpBurnMessageIdV1(
        SCCP_DOMAIN_TON,
        destDomain,
        nonce,
        sccp.soraAssetId,
        jettonAmount,
        recipient32,
    );
}

get fun get_sccp_inbound_message_id(sourceDomain: uint32, burnNonce: uint64, jettonAmount: coins, recipient32: uint256): uint256 {
    val storage = lazy MinterStorage.load();
    val sccp = lazy storage.sccp.load();
    return sccpBurnMessageIdV1(
        sourceDomain,
        SCCP_DOMAIN_TON,
        burnNonce,
        sccp.soraAssetId,
        jettonAmount,
        recipient32,
    );
}
