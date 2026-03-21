// SPDX-License-Identifier: BSD-4-Clause
pragma solidity ^0.8.23;

import {SccpCodec} from "../SccpCodec.sol";

/// @notice Test helper exposing SCCP codec functions for off-chain test runners.
contract SccpCodecTest {
    function encodeBurnPayloadV1(
        uint32 sourceDomain,
        uint32 destDomain,
        uint64 nonce,
        bytes32 soraAssetId,
        uint128 amount,
        bytes32 recipient
    ) external pure returns (bytes memory payload) {
        SccpCodec.BurnPayloadV1 memory p = SccpCodec.BurnPayloadV1({
            version: 1,
            sourceDomain: sourceDomain,
            destDomain: destDomain,
            nonce: nonce,
            soraAssetId: soraAssetId,
            amount: amount,
            recipient: recipient
        });
        payload = SccpCodec.encodeBurnPayloadV1(p);
    }

    function burnMessageId(bytes calldata payload) external pure returns (bytes32) {
        return SccpCodec.burnMessageId(payload);
    }

    function encodeTokenAddPayloadV1(
        uint32 targetDomain,
        uint64 nonce,
        bytes32 soraAssetId,
        uint8 decimals,
        bytes32 name,
        bytes32 symbol
    ) external pure returns (bytes memory payload) {
        SccpCodec.TokenAddPayloadV1 memory p = SccpCodec.TokenAddPayloadV1({
            version: 1,
            targetDomain: targetDomain,
            nonce: nonce,
            soraAssetId: soraAssetId,
            decimals: decimals,
            name: name,
            symbol: symbol
        });
        payload = SccpCodec.encodeTokenAddPayloadV1(p);
    }

    function encodeTokenPausePayloadV1(uint32 targetDomain, uint64 nonce, bytes32 soraAssetId)
        external
        pure
        returns (bytes memory payload)
    {
        SccpCodec.TokenControlPayloadV1 memory p =
            SccpCodec.TokenControlPayloadV1({ version: 1, targetDomain: targetDomain, nonce: nonce, soraAssetId: soraAssetId });
        payload = SccpCodec.encodeTokenPausePayloadV1(p);
    }

    function encodeTokenResumePayloadV1(uint32 targetDomain, uint64 nonce, bytes32 soraAssetId)
        external
        pure
        returns (bytes memory payload)
    {
        SccpCodec.TokenControlPayloadV1 memory p =
            SccpCodec.TokenControlPayloadV1({ version: 1, targetDomain: targetDomain, nonce: nonce, soraAssetId: soraAssetId });
        payload = SccpCodec.encodeTokenResumePayloadV1(p);
    }

    function tokenAddMessageId(bytes calldata payload) external pure returns (bytes32) {
        return SccpCodec.tokenAddMessageId(payload);
    }

    function tokenPauseMessageId(bytes calldata payload) external pure returns (bytes32) {
        return SccpCodec.tokenPauseMessageId(payload);
    }

    function tokenResumeMessageId(bytes calldata payload) external pure returns (bytes32) {
        return SccpCodec.tokenResumeMessageId(payload);
    }

    function decodeBurnPayloadV1(bytes calldata payload)
        external
        pure
        returns (
            uint8 version,
            uint32 sourceDomain,
            uint32 destDomain,
            uint64 nonce,
            bytes32 soraAssetId,
            uint128 amount,
            bytes32 recipient
        )
    {
        SccpCodec.BurnPayloadV1 memory p = SccpCodec.decodeBurnPayloadV1(payload);
        return (p.version, p.sourceDomain, p.destDomain, p.nonce, p.soraAssetId, p.amount, p.recipient);
    }

    function decodeTokenAddPayloadV1(bytes calldata payload)
        external
        pure
        returns (
            uint8 version,
            uint32 targetDomain,
            uint64 nonce,
            bytes32 soraAssetId,
            uint8 decimals,
            bytes32 name,
            bytes32 symbol
        )
    {
        SccpCodec.TokenAddPayloadV1 memory p = SccpCodec.decodeTokenAddPayloadV1(payload);
        return (p.version, p.targetDomain, p.nonce, p.soraAssetId, p.decimals, p.name, p.symbol);
    }

    function decodeTokenPausePayloadV1(bytes calldata payload)
        external
        pure
        returns (uint8 version, uint32 targetDomain, uint64 nonce, bytes32 soraAssetId)
    {
        SccpCodec.TokenControlPayloadV1 memory p = SccpCodec.decodeTokenPausePayloadV1(payload);
        return (p.version, p.targetDomain, p.nonce, p.soraAssetId);
    }

    function decodeTokenResumePayloadV1(bytes calldata payload)
        external
        pure
        returns (uint8 version, uint32 targetDomain, uint64 nonce, bytes32 soraAssetId)
    {
        SccpCodec.TokenControlPayloadV1 memory p = SccpCodec.decodeTokenResumePayloadV1(payload);
        return (p.version, p.targetDomain, p.nonce, p.soraAssetId);
    }
}
