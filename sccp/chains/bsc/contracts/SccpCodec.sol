// SPDX-License-Identifier: BSD-4-Clause
pragma solidity ^0.8.23;

/// @notice SCALE codec helpers for SCCP payloads.
library SccpCodec {
    bytes internal constant SCCP_MSG_PREFIX_BURN_V1 = "sccp:burn:v1";
    bytes internal constant SCCP_MSG_PREFIX_TOKEN_ADD_V1 = "sccp:token:add:v1";
    bytes internal constant SCCP_MSG_PREFIX_TOKEN_PAUSE_V1 = "sccp:token:pause:v1";
    bytes internal constant SCCP_MSG_PREFIX_TOKEN_RESUME_V1 = "sccp:token:resume:v1";

    uint256 internal constant BURN_PAYLOAD_V1_LEN = 97;
    uint256 internal constant TOKEN_ADD_PAYLOAD_V1_LEN = 110;
    uint256 internal constant TOKEN_CONTROL_PAYLOAD_V1_LEN = 45;

    error InvalidPayloadLength(uint256 len);

    struct BurnPayloadV1 {
        uint8 version;
        uint32 sourceDomain;
        uint32 destDomain;
        uint64 nonce;
        bytes32 soraAssetId;
        uint128 amount;
        bytes32 recipient;
    }

    struct TokenAddPayloadV1 {
        uint8 version;
        uint32 targetDomain;
        uint64 nonce;
        bytes32 soraAssetId;
        uint8 decimals;
        bytes32 name;
        bytes32 symbol;
    }

    struct TokenControlPayloadV1 {
        uint8 version;
        uint32 targetDomain;
        uint64 nonce;
        bytes32 soraAssetId;
    }

    function burnMessageId(bytes memory payload) internal pure returns (bytes32) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(bytes.concat(SCCP_MSG_PREFIX_BURN_V1, payload));
    }

    function tokenAddMessageId(bytes memory payload) internal pure returns (bytes32) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(bytes.concat(SCCP_MSG_PREFIX_TOKEN_ADD_V1, payload));
    }

    function tokenPauseMessageId(bytes memory payload) internal pure returns (bytes32) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(bytes.concat(SCCP_MSG_PREFIX_TOKEN_PAUSE_V1, payload));
    }

    function tokenResumeMessageId(bytes memory payload) internal pure returns (bytes32) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(bytes.concat(SCCP_MSG_PREFIX_TOKEN_RESUME_V1, payload));
    }

    function encodeBurnPayloadV1(BurnPayloadV1 memory p) internal pure returns (bytes memory payload) {
        payload = new bytes(BURN_PAYLOAD_V1_LEN);
        payload[0] = bytes1(p.version);
        _writeLe32(payload, 1, p.sourceDomain);
        _writeLe32(payload, 5, p.destDomain);
        _writeLe64(payload, 9, p.nonce);
        _writeBytes32(payload, 17, p.soraAssetId);
        _writeLe128(payload, 49, p.amount);
        _writeBytes32(payload, 65, p.recipient);
    }

    function encodeTokenAddPayloadV1(TokenAddPayloadV1 memory p) internal pure returns (bytes memory payload) {
        payload = new bytes(TOKEN_ADD_PAYLOAD_V1_LEN);
        payload[0] = bytes1(p.version);
        _writeLe32(payload, 1, p.targetDomain);
        _writeLe64(payload, 5, p.nonce);
        _writeBytes32(payload, 13, p.soraAssetId);
        payload[45] = bytes1(p.decimals);
        _writeBytes32(payload, 46, p.name);
        _writeBytes32(payload, 78, p.symbol);
    }

    function encodeTokenPausePayloadV1(TokenControlPayloadV1 memory p) internal pure returns (bytes memory payload) {
        payload = new bytes(TOKEN_CONTROL_PAYLOAD_V1_LEN);
        payload[0] = bytes1(p.version);
        _writeLe32(payload, 1, p.targetDomain);
        _writeLe64(payload, 5, p.nonce);
        _writeBytes32(payload, 13, p.soraAssetId);
    }

    function encodeTokenResumePayloadV1(TokenControlPayloadV1 memory p) internal pure returns (bytes memory payload) {
        payload = encodeTokenPausePayloadV1(p);
    }

    function decodeBurnPayloadV1(bytes calldata payload) internal pure returns (BurnPayloadV1 memory p) {
        if (payload.length != BURN_PAYLOAD_V1_LEN) revert InvalidPayloadLength(payload.length);
        p.version = uint8(payload[0]);
        p.sourceDomain = _readLe32(payload, 1);
        p.destDomain = _readLe32(payload, 5);
        p.nonce = _readLe64(payload, 9);
        p.soraAssetId = _readBytes32(payload, 17);
        p.amount = _readLe128(payload, 49);
        p.recipient = _readBytes32(payload, 65);
    }

    function decodeTokenAddPayloadV1(bytes calldata payload) internal pure returns (TokenAddPayloadV1 memory p) {
        if (payload.length != TOKEN_ADD_PAYLOAD_V1_LEN) revert InvalidPayloadLength(payload.length);
        p.version = uint8(payload[0]);
        p.targetDomain = _readLe32(payload, 1);
        p.nonce = _readLe64(payload, 5);
        p.soraAssetId = _readBytes32(payload, 13);
        p.decimals = uint8(payload[45]);
        p.name = _readBytes32(payload, 46);
        p.symbol = _readBytes32(payload, 78);
    }

    function decodeTokenPausePayloadV1(bytes calldata payload) internal pure returns (TokenControlPayloadV1 memory p) {
        if (payload.length != TOKEN_CONTROL_PAYLOAD_V1_LEN) revert InvalidPayloadLength(payload.length);
        p.version = uint8(payload[0]);
        p.targetDomain = _readLe32(payload, 1);
        p.nonce = _readLe64(payload, 5);
        p.soraAssetId = _readBytes32(payload, 13);
    }

    function decodeTokenResumePayloadV1(bytes calldata payload) internal pure returns (TokenControlPayloadV1 memory p) {
        p = decodeTokenPausePayloadV1(payload);
    }

    function _writeBytes32(bytes memory b, uint256 off, bytes32 v) private pure {
        assembly {
            mstore(add(add(b, 32), off), v)
        }
    }

    function _writeLe32(bytes memory b, uint256 off, uint32 v) private pure {
        assembly {
            let ptr := add(add(b, 32), off)
            mstore8(ptr, and(v, 0xff))
            mstore8(add(ptr, 1), and(shr(8, v), 0xff))
            mstore8(add(ptr, 2), and(shr(16, v), 0xff))
            mstore8(add(ptr, 3), and(shr(24, v), 0xff))
        }
    }

    function _writeLe64(bytes memory b, uint256 off, uint64 v) private pure {
        assembly {
            let ptr := add(add(b, 32), off)
            mstore8(ptr, and(v, 0xff))
            mstore8(add(ptr, 1), and(shr(8, v), 0xff))
            mstore8(add(ptr, 2), and(shr(16, v), 0xff))
            mstore8(add(ptr, 3), and(shr(24, v), 0xff))
            mstore8(add(ptr, 4), and(shr(32, v), 0xff))
            mstore8(add(ptr, 5), and(shr(40, v), 0xff))
            mstore8(add(ptr, 6), and(shr(48, v), 0xff))
            mstore8(add(ptr, 7), and(shr(56, v), 0xff))
        }
    }

    function _writeLe128(bytes memory b, uint256 off, uint128 v) private pure {
        assembly {
            let ptr := add(add(b, 32), off)
            mstore8(ptr, and(v, 0xff))
            mstore8(add(ptr, 1), and(shr(8, v), 0xff))
            mstore8(add(ptr, 2), and(shr(16, v), 0xff))
            mstore8(add(ptr, 3), and(shr(24, v), 0xff))
            mstore8(add(ptr, 4), and(shr(32, v), 0xff))
            mstore8(add(ptr, 5), and(shr(40, v), 0xff))
            mstore8(add(ptr, 6), and(shr(48, v), 0xff))
            mstore8(add(ptr, 7), and(shr(56, v), 0xff))
            mstore8(add(ptr, 8), and(shr(64, v), 0xff))
            mstore8(add(ptr, 9), and(shr(72, v), 0xff))
            mstore8(add(ptr, 10), and(shr(80, v), 0xff))
            mstore8(add(ptr, 11), and(shr(88, v), 0xff))
            mstore8(add(ptr, 12), and(shr(96, v), 0xff))
            mstore8(add(ptr, 13), and(shr(104, v), 0xff))
            mstore8(add(ptr, 14), and(shr(112, v), 0xff))
            mstore8(add(ptr, 15), and(shr(120, v), 0xff))
        }
    }

    function _readBytes32(bytes calldata b, uint256 off) private pure returns (bytes32 out) {
        assembly {
            out := calldataload(add(b.offset, off))
        }
    }

    function _readLe32(bytes calldata b, uint256 off) private pure returns (uint32 v) {
        v =
            uint32(uint8(b[off])) |
            (uint32(uint8(b[off + 1])) << 8) |
            (uint32(uint8(b[off + 2])) << 16) |
            (uint32(uint8(b[off + 3])) << 24);
    }

    function _readLe64(bytes calldata b, uint256 off) private pure returns (uint64 v) {
        v =
            uint64(uint8(b[off])) |
            (uint64(uint8(b[off + 1])) << 8) |
            (uint64(uint8(b[off + 2])) << 16) |
            (uint64(uint8(b[off + 3])) << 24) |
            (uint64(uint8(b[off + 4])) << 32) |
            (uint64(uint8(b[off + 5])) << 40) |
            (uint64(uint8(b[off + 6])) << 48) |
            (uint64(uint8(b[off + 7])) << 56);
    }

    function _readLe128(bytes calldata b, uint256 off) private pure returns (uint128 v) {
        for (uint256 i = 0; i < 16; i++) {
            v |= uint128(uint256(uint8(b[off + i])) << (8 * i));
        }
    }

}
