// SPDX-License-Identifier: BSD-4-Clause
pragma solidity ^0.8.23;

import {SccpCodec} from "../SccpCodec.sol";

contract EchidnaSccpCodec {
    bytes private lastPayload;
    bytes private lastBadPayload;
    bytes32 private lastMessageId;
    bool private hasPayload;

    function step(
        uint8 version,
        uint32 sourceDomain,
        uint32 destDomain,
        uint64 nonce,
        bytes32 soraAssetId,
        uint128 amount,
        bytes32 recipient
    ) public {
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
        SccpCodec.BurnPayloadV1 memory decoded = this.decodeExternal(payload);

        lastPayload = payload;
        lastMessageId = SccpCodec.burnMessageId(payload);
        hasPayload = true;

        assert(decoded.version == version);
        assert(decoded.sourceDomain == sourceDomain);
        assert(decoded.destDomain == destDomain);
        assert(decoded.nonce == nonce);
        assert(decoded.soraAssetId == soraAssetId);
        assert(decoded.amount == amount);
        assert(decoded.recipient == recipient);
    }

    function stepBad(bytes calldata payload) public {
        lastBadPayload = payload;
    }

    function decodeExternal(bytes calldata payload) external pure returns (SccpCodec.BurnPayloadV1 memory) {
        return SccpCodec.decodeBurnPayloadV1(payload);
    }

    // forge-lint: disable-next-line(mixed-case-function)
    function echidna_payload_length_is_fixed() public view returns (bool) {
        return !hasPayload || lastPayload.length == 97;
    }

    // forge-lint: disable-next-line(mixed-case-function)
    function echidna_message_id_is_stable_for_last_payload() public view returns (bool) {
        return !hasPayload || lastMessageId == SccpCodec.burnMessageId(lastPayload);
    }

    // forge-lint: disable-next-line(mixed-case-function)
    function echidna_rejects_non_97_decode_length() public view returns (bool) {
        if (lastBadPayload.length == 97) {
            return true;
        }
        try this.decodeExternal(lastBadPayload) returns (SccpCodec.BurnPayloadV1 memory) {
            return false;
        } catch {
            return true;
        }
    }
}
