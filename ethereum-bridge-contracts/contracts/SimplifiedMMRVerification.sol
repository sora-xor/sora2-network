// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.15;

import "./interfaces/ISimplifiedMMRVerification.sol";

contract SimplifiedMMRVerification is ISimplifiedMMRVerification {
    function verifyInclusionProof(
        bytes32 root,
        bytes32 leafNodeHash,
        SimplifiedMMRProof memory proof
    ) external pure override returns (bool) {
        require(proof.merkleProofItems.length < 64);

        return
            root ==
            calculateMerkleRoot(
                leafNodeHash,
                proof.merkleProofItems,
                proof.merkleProofOrderBitField
            );
    }

    // Get the value of the bit at the given 'index' in 'self'.
    // index should be validated beforehand to make sure it is less than 64
    function bit(uint64 self, uint256 index) internal pure returns (bool) {
        return uint8((self >> index) & 1) == 1;
    }

    function calculateMerkleRoot(
        bytes32 leafNodeHash,
        bytes32[] memory merkleProofItems,
        uint64 merkleProofOrderBitField
    ) public pure override returns (bytes32 currentHash) {
        currentHash = leafNodeHash;
        uint256 length = merkleProofItems.length;
        for (
            uint256 currentPosition = 0;
            currentPosition < length;
            currentPosition++
        ) {
            bool isSiblingLeft = bit(merkleProofOrderBitField, currentPosition);
            bytes32 sibling = merkleProofItems[currentPosition];
            currentHash = isSiblingLeft
                ? keccak256(abi.encodePacked(sibling, currentHash))
                : keccak256(abi.encodePacked(currentHash, sibling));
        }

        return currentHash;
    }
}
