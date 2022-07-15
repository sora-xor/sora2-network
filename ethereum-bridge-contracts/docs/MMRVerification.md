## `MMRVerification`



MMRVerification library for MMR inclusion proofs generated
     by https://github.com/nervosnetwork/merkle-mountain-range.

                 Sample 7-leaf MMR:

         Height 3 |      7
         Height 2 |   3      6     10
         Height 1 | 1  2   4  5   8  9    11
                  | |--|---|--|---|--|-----|-
     Leaf indexes | 0  1   2  3   4  5     6

     General definitions:
     - Height:         the height of the tree.
     - Width:          the number of leaves in the tree.
     - Size:           the number of nodes in the tree.
     - Nodes:          an item in the tree. A node is a leaf or a parent. Nodes' positions are ordered from 1
                       to size in the order that they were added to the tree.
     - Leaf Index:     the leaf's location in an ordered array of all leaf nodes. Because Solidity interprets
                       0 as null, this MMR implementation internally converts leaf index to leaf position.
     - Parent Node:    leaf nodes are hashed together into parent nodes. To maintain the tree's structure,
                       parent nodes are hashed together until they form a mountain with a peak.
     - Mountain Peak:  the local root of a mountain; it has a greater height than other nodes in the mountain.
     - MMR root:       hashing each peak's hash together right-to-left gives the MMR root.

     Our 7-leaf MMR has:
     - Height:          3
     - Size:            11
     - Nodes:          [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
     - Leaf Indexes:   [0, 1, 2, 3, 4, 5, 6] which correspond to nodes [1, 2, 4, 5, 8, 9, 11]
     - Parent Nodes:   [3, 6, 7, 10, 11]
     - Mountain peaks: [7, 10, 11]
     - MMR root:       hash(hash(11, 10), 7)


### `verifyInclusionProof(bytes32 root, bytes32 leafNodeHash, uint256 leafIndex, uint256 leafCount, bytes32[] proofItems) → bool` (public)



Verify an MMR inclusion proof for a leaf at a given index.

### `calculatePeakRoot(uint256 numLeftPeaks, bytes32 leafNodeHash, uint256 leafPos, uint256 peakPos, bytes32[] proofItems) → bytes32` (public)



Calculate a leaf's mountain peak based on it's hash, it's position,
     the mountain peak's position, and the proof contents.

### `mountainHeight(uint256 size) → uint8` (public)



It returns the height of the highest peak

### `heightAt(uint256 index) → uint8 height` (public)



It returns the height of the index

### `isLeaf(uint256 index) → bool` (public)



It returns whether the index is the leaf node or not

### `getPeakPositions(uint256 width) → uint256[] peakPositions` (public)



It returns positions of all peaks

### `numOfPeaks(uint256 numLeaves) → uint256 numPeaks` (public)



Return number of peaks from number of leaves

### `getSize(uint256 numLeaves) → uint256` (internal)



Return MMR size from number of leaves

### `bitCount(uint256 n) → uint256` (internal)



Counts the number of 1s in the binary representation of an integer

### `leafIndexToPos(uint256 index) → uint256` (internal)



Return position of leaf at given leaf index

### `leafIndexToMmrSize(uint256 index) → uint256` (internal)



Return

### `trailingZeros(uint256 x) → uint256` (internal)



Counts the number of trailing 0s in the binary representation of an integer

### `parentOffset(uint256 height) → uint256 num` (internal)



Return parent offset at a given height

### `siblingOffset(uint256 height) → uint256 num` (internal)



Return sibling offset at a given height


