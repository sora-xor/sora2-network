// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

// Something that can reward a relayer
interface ISimplifiedMMRProof {
    struct SimplifiedMMRProof {
        bytes32[] merkleProofItems;
        uint64 merkleProofOrderBitField;
    }
}
