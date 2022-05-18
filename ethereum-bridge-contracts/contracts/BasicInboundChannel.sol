// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "./BeefyLightClient.sol";
import "./SimplifiedMMRVerification.sol";
import "./ScaleCodec.sol";

contract BasicInboundChannel {
    using ScaleCodec for uint256;
    using ScaleCodec for uint64;
    using ScaleCodec for uint32;
    using ScaleCodec for uint16;
    uint256 public constant MAX_GAS_PER_MESSAGE = 2000000;
    uint256 public constant GAS_BUFFER = 60000;

    uint64 public nonce;

    BeefyLightClient public beefyLightClient;

    struct Message {
        address target;
        uint64 nonce;
        bytes payload;
    }

    event MessageDispatched(uint64 nonce, bool result);

    constructor(BeefyLightClient _beefyLightClient) {
        nonce = 0;
        beefyLightClient = _beefyLightClient;
    }

    function submit(
        Message[] calldata _messages,
        LeafBytes calldata _leafBytes,
        SimplifiedMMRProof calldata proof
    ) public {
        verifyMerkleLeaf(_messages, _leafBytes, proof);

        // Require there is enough gas to play all messages
        require(
            gasleft() >= (_messages.length * MAX_GAS_PER_MESSAGE) + GAS_BUFFER,
            "insufficient gas for delivery of all messages"
        );

        processMessages(_messages);
    }

    struct LeafBytes {
        bytes digestPrefix;
        bytes digestSuffix;
        bytes leafPrefix;
    }

    function verifyMerkleLeaf(
        Message[] calldata _messages,
        LeafBytes calldata _leafBytes,
        SimplifiedMMRProof calldata proof
    ) internal view {
        bytes32 commitment = keccak256(abi.encode(_messages));
        bytes32 digestHash = keccak256(
            bytes.concat(
                _leafBytes.digestPrefix,
                uint32(block.chainid).encode32(),
                bytes1(uint8(0)),
                commitment,
                _leafBytes.digestSuffix
            )
        );
        delete commitment;
        bytes32 leafHash = keccak256(
            bytes.concat(_leafBytes.leafPrefix, digestHash)
        );
        delete digestHash;

        require(
            beefyLightClient.verifyBeefyMerkleLeaf(leafHash, proof),
            "Invalid proof"
        );
    }

    function processMessages(Message[] calldata _messages) internal {
        for (uint256 i = 0; i < _messages.length; i++) {
            // Check message nonce is correct and increment nonce for replay protection
            require(_messages[i].nonce == nonce + 1, "invalid nonce");

            nonce = nonce + 1;

            // Deliver the message to the target
            (bool success, ) = _messages[i].target.call{
                value: 0,
                gas: MAX_GAS_PER_MESSAGE
            }(_messages[i].payload);

            emit MessageDispatched(_messages[i].nonce, success);
        }
    }
}
