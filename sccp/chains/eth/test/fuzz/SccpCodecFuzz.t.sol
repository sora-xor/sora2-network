// SPDX-License-Identifier: BSD-4-Clause
pragma solidity ^0.8.23;

import {SccpCodec} from "../../contracts/SccpCodec.sol";

contract SccpCodecFuzz {
    function decodeExternal(bytes calldata payload) external pure returns (SccpCodec.BurnPayloadV1 memory) {
        return SccpCodec.decodeBurnPayloadV1(payload);
    }

    function decodeViaExternal(bytes memory payload)
        internal
        view
        returns (bool ok, SccpCodec.BurnPayloadV1 memory decoded)
    {
        bytes memory callData = abi.encodeWithSelector(this.decodeExternal.selector, payload);
        bytes memory returnData;
        (ok, returnData) = address(this).staticcall(callData);
        if (ok) {
            decoded = abi.decode(returnData, (SccpCodec.BurnPayloadV1));
        }
    }

    function testFuzz_encode_decode_roundtrip(
        uint8 version,
        uint32 sourceDomain,
        uint32 destDomain,
        uint64 nonce,
        bytes32 soraAssetId,
        uint128 amount,
        bytes32 recipient
    ) public view {
        SccpCodec.BurnPayloadV1 memory p = SccpCodec.BurnPayloadV1({
            version: version,
            sourceDomain: sourceDomain,
            destDomain: destDomain,
            nonce: nonce,
            soraAssetId: soraAssetId,
            amount: amount,
            recipient: recipient
        });

        bytes memory payload = SccpCodec.encodeBurnPayloadV1(p);
        assert(payload.length == 97);

        (bool ok, SccpCodec.BurnPayloadV1 memory decoded) = decodeViaExternal(payload);
        assert(ok);
        assert(decoded.version == version);
        assert(decoded.sourceDomain == sourceDomain);
        assert(decoded.destDomain == destDomain);
        assert(decoded.nonce == nonce);
        assert(decoded.soraAssetId == soraAssetId);
        assert(decoded.amount == amount);
        assert(decoded.recipient == recipient);
    }

    function testFuzz_message_id_nonce_sensitivity(
        uint8 version,
        uint32 sourceDomain,
        uint32 destDomain,
        uint64 nonce,
        bytes32 soraAssetId,
        uint128 amount,
        bytes32 recipient
    ) public pure {
        if (nonce == type(uint64).max) {
            return;
        }

        SccpCodec.BurnPayloadV1 memory a = SccpCodec.BurnPayloadV1({
            version: version,
            sourceDomain: sourceDomain,
            destDomain: destDomain,
            nonce: nonce,
            soraAssetId: soraAssetId,
            amount: amount,
            recipient: recipient
        });
        SccpCodec.BurnPayloadV1 memory b = SccpCodec.BurnPayloadV1({
            version: version,
            sourceDomain: sourceDomain,
            destDomain: destDomain,
            nonce: nonce + 1,
            soraAssetId: soraAssetId,
            amount: amount,
            recipient: recipient
        });

        bytes32 idA = SccpCodec.burnMessageId(SccpCodec.encodeBurnPayloadV1(a));
        bytes32 idB = SccpCodec.burnMessageId(SccpCodec.encodeBurnPayloadV1(b));
        assert(idA != idB);
    }

    function testFuzz_decode_rejects_non_canonical_length(bytes memory payload) public view {
        if (payload.length == 97) {
            return;
        }
        (bool ok,) = decodeViaExternal(payload);
        assert(!ok);
    }
}
