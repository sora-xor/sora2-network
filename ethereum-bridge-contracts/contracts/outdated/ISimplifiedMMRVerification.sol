// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "./ISimplifiedMMRProof.sol";

interface ISimplifiedMMRVerification is ISimplifiedMMRProof{
    function verifyInclusionProof(
        bytes32 root,
        bytes32 leafNodeHash,
        SimplifiedMMRProof memory proof
    ) external pure returns (bool);

    function calculateMerkleRoot(
        bytes32 leafNodeHash,
        bytes32[] memory merkleProofItems,
        uint64 merkleProofOrderBitField
    ) external pure returns (bytes32 currentHash);
}
