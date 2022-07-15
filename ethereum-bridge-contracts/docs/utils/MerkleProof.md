## `MerkleProof`






### `verifyMerkleLeafAtPosition(bytes32 root, bytes32 leaf, uint256 pos, uint256 width, bytes32[] proof) → bool` (public)

Verify that a specific leaf element is part of the Merkle Tree at a specific position in the tree





### `computeRootFromProofAndSide(bytes32 leaf, bytes32[] proof, bool[] side) → bytes32` (public)

Compute the root of a MMR from a leaf and proof





### `computeRootFromProofAtPosition(bytes32 leaf, uint256 pos, uint256 width, bytes32[] proof) → bytes32` (public)






