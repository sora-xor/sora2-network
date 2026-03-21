// SPDX-License-Identifier: BSD-4-Clause
pragma solidity ^0.8.23;

import {ISccpVerifier} from "../ISccpVerifier.sol";
import {SccpCodec} from "../SccpCodec.sol";

/// @notice Trustless on-chain verifier for SORA SCCP burns using a BEEFY+MMR light client.
///
/// Model:
/// - Anyone can import finalized SORA MMR roots by submitting:
///   - a BEEFY commitment (payload = MMR root, block number, validator set id)
///   - >=2/3 validator signatures + merkle proofs against the stored validator set root
///   - an MMR leaf + simplified inclusion proof under that MMR root (to transition validator sets)
/// - Once an MMR root is imported, SCCP burns can be verified by proving:
///   - the burn `messageId` is included as an `AuxiliaryDigestItem::Commitment` in the SORA
///     auxiliary digest (hashed into the MMR leaf extra data)
///
/// This contract is the shared `ISccpVerifier` implementation for SCCP 20-byte-address domains
/// such as Ethereum, BSC, and TRON.
contract SoraBeefyLightClientVerifier is ISccpVerifier {
    using SccpCodec for bytes;

    // SCCP domains (must match router constants).
    uint32 public constant DOMAIN_SORA = 0;

    // Leaf provider digest commitment network id sentinel (matches SORA pallet `SCCP_DIGEST_NETWORK_ID`).
    uint32 internal constant SCCP_DIGEST_NETWORK_ID = 0x53434350; // 'SCCP'

    // SCALE enum discriminants (bridge-types v1.0.27):
    uint8 internal constant AUX_DIGEST_ITEM_COMMITMENT = 0;
    uint8 internal constant GENERIC_NETWORK_ID_EVM_LEGACY = 2;
    uint8 internal constant GENERIC_NETWORK_ID_EVM = 0;
    uint8 internal constant GENERIC_NETWORK_ID_SUB = 1;
    uint8 internal constant GENERIC_NETWORK_ID_TON = 3;
    uint256 internal constant SECP256K1N_HALF_ORDER =
        0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0;

    uint32 public constant MMR_ROOT_HISTORY_SIZE = 30;

    error InvalidValidatorSetId();
    error PayloadBlocknumberTooOld();
    error NotEnoughValidatorSignatures();
    error InvalidValidatorProof();
    error ValidatorSetIncorrectPosition();
    error InvalidSignature();
    error InvalidMMRProof();
    error OnlySelf();

    event Initialized(uint64 latestBeefyBlock, uint64 currentValidatorSetId, uint64 nextValidatorSetId);
    event NewMMRRoot(bytes32 indexed mmrRoot, uint64 blockNumber);
    event ValidatorSetsUpdated(uint64 currentId, uint64 nextId);

    struct ValidatorSet {
        uint64 id;
        uint32 len;
        bytes32 root;
    }

    /// @dev Mirror of `sp_beefy::Commitment<u32>` with payload restricted to the standard single entry:
    ///      (`known_payloads::MMR_ROOT_ID` = "mh") -> SCALE(H256 mmr_root).
    struct Commitment {
        bytes32 mmrRoot;
        uint32 blockNumber;
        uint64 validatorSetId;
    }

    struct ValidatorProof {
        bytes[] signatures; // 65 bytes each: r[32]||s[32]||v[1] with v in {0,1,27,28}
        uint256[] positions; // index in validator set
        address[] publicKeys; // Ethereum address for validator
        bytes32[][] publicKeyMerkleProofs; // ordered binary merkle proofs (Substrate `binary_merkle_tree`)
    }

    struct MmrProof {
        /// @dev Leaf index in the SORA MMR (0-based).
        uint64 leafIndex;
        /// @dev Total leaf count at the time of the proof (used to derive MMR size and peaks).
        uint64 leafCount;
        bytes32[] items;
    }

    struct MmrRootCalcState {
        uint64 leafPos;
        uint256 peaksLen;
        uint256 proofIdx;
        bool leafUsed;
    }

    /// @dev SCALE of `sp_beefy::mmr::MmrLeaf<u32, H256, H256, LeafExtraData<H256,H256>>` (fixed-width).
    struct MmrLeaf {
        uint8 version;
        uint32 parentNumber;
        bytes32 parentHash;
        uint64 nextAuthoritySetId;
        uint32 nextAuthoritySetLen;
        bytes32 nextAuthoritySetRoot;
        bytes32 randomSeed;
        bytes32 digestHash;
    }

    uint64 public latestBeefyBlock;

    ValidatorSet public currentValidatorSet;
    ValidatorSet public nextValidatorSet;

    bytes32[MMR_ROOT_HISTORY_SIZE] public mmrRoots;
    uint256 public mmrRootsPos;
    mapping(bytes32 => bool) public knownMmrRoot;

    /// @notice Constructor bootstrap for the BEEFY light client.
    ///
    /// The initial validator sets and `latestBeefyBlock` must be sourced from SORA chain state.
    constructor(uint64 latestBeefyBlock_, ValidatorSet memory current_, ValidatorSet memory next_) {
        if (current_.len == 0 || next_.len == 0) revert InvalidValidatorProof();
        if (current_.root == bytes32(0) || next_.root == bytes32(0)) revert InvalidValidatorProof();
        if (next_.id <= current_.id) revert InvalidValidatorSetId();
        latestBeefyBlock = latestBeefyBlock_;
        currentValidatorSet = current_;
        nextValidatorSet = next_;
        emit Initialized(latestBeefyBlock_, current_.id, next_.id);
    }

    /// @notice Import a new finalized MMR root from SORA by verifying a BEEFY commitment.
    /// @dev Permissionless.
    function submitSignatureCommitment(
        Commitment calldata commitment,
        ValidatorProof calldata validatorProof,
        MmrLeaf calldata latestMmrLeaf,
        MmrProof calldata proof
    ) external {
        // Basic freshness check (fail fast).
        if (uint64(commitment.blockNumber) <= latestBeefyBlock) revert PayloadBlocknumberTooOld();

        ValidatorSet memory vset;
        if (commitment.validatorSetId == currentValidatorSet.id) {
            vset = currentValidatorSet;
        } else if (commitment.validatorSetId == nextValidatorSet.id) {
            vset = nextValidatorSet;
        } else {
            revert InvalidValidatorSetId();
        }

        _verifyCommitmentSignatures(commitment, validatorProof, vset);

        // Validator-set transitions must be derived from the latest finalized MMR leaf, not an arbitrary
        // historical leaf under the same root.
        if (proof.leafCount == 0 || (uint256(proof.leafIndex) + 1) != uint256(proof.leafCount)) {
            revert InvalidMMRProof();
        }

        // Verify the provided MMR leaf is included under the payload root.
        bytes32 leafHash = hashLeaf(latestMmrLeaf);
        if (mmrProofRoot(leafHash, proof) != commitment.mmrRoot) revert InvalidMMRProof();

        _addKnownMmrRoot(commitment.mmrRoot);
        latestBeefyBlock = uint64(commitment.blockNumber);
        emit NewMMRRoot(commitment.mmrRoot, uint64(commitment.blockNumber));

        // Apply validator set changes (if any) from the leaf.
        ValidatorSet memory newVset = ValidatorSet({
            id: latestMmrLeaf.nextAuthoritySetId,
            len: latestMmrLeaf.nextAuthoritySetLen,
            root: latestMmrLeaf.nextAuthoritySetRoot
        });
        if (newVset.id == nextValidatorSet.id) {
            if (newVset.len != nextValidatorSet.len || newVset.root != nextValidatorSet.root) {
                revert InvalidValidatorProof();
            }
        } else if (newVset.id > nextValidatorSet.id) {
            if (newVset.len == 0 || newVset.root == bytes32(0)) revert InvalidValidatorProof();
            currentValidatorSet = nextValidatorSet;
            nextValidatorSet = newVset;
            emit ValidatorSetsUpdated(currentValidatorSet.id, nextValidatorSet.id);
        }
    }

    /// @notice Verify that `payload`'s `messageId` is committed into a finalized SORA MMR root.
    /// @dev Returns `false` (not revert) on failure per `ISccpVerifier` expectations.
    ///
    /// Proof format: `abi.encode(uint64 leafIndex, uint64 leafCount, bytes32[] items, MmrLeaf leaf, bytes digestScale)`
    function verifyBurnProof(
        uint32 sourceDomain,
        bytes32 messageId,
        bytes calldata payload,
        bytes calldata proof
    ) external view returns (bool) {
        // `sourceDomain` is the burn origin domain (from payload). This verifier only attests that
        // SORA has committed `messageId` into its auxiliary digest (BEEFY+MMR finalized).
        // It therefore supports SCCP burns from any source domain, as long as SORA committed them.
        sourceDomain;
        if (SccpCodec.burnMessageId(payload) != messageId) return false;
        return _verifyMessageProof(messageId, proof);
    }

    function verifyTokenAddProof(bytes32 messageId, bytes calldata payload, bytes calldata proof)
        external
        view
        returns (bool)
    {
        if (SccpCodec.tokenAddMessageId(payload) != messageId) return false;
        return _verifyMessageProof(messageId, proof);
    }

    function verifyTokenPauseProof(bytes32 messageId, bytes calldata payload, bytes calldata proof)
        external
        view
        returns (bool)
    {
        if (SccpCodec.tokenPauseMessageId(payload) != messageId) return false;
        return _verifyMessageProof(messageId, proof);
    }

    function verifyTokenResumeProof(bytes32 messageId, bytes calldata payload, bytes calldata proof)
        external
        view
        returns (bool)
    {
        if (SccpCodec.tokenResumeMessageId(payload) != messageId) return false;
        return _verifyMessageProof(messageId, proof);
    }

    function _verifyMessageProof(bytes32 messageId, bytes calldata proofBytes) internal view returns (bool) {
        // Keep this function non-reverting by catching decode errors.
        try this._verifyMessageProofInternal(messageId, proofBytes) returns (bool ok) {
            return ok;
        } catch {
            return false;
        }
    }

    function _verifyMessageProofInternal(bytes32 messageId, bytes calldata proofBytes) external view returns (bool) {
        if (msg.sender != address(this)) revert OnlySelf();

        (uint64 leafIndex, uint64 leafCount, bytes32[] memory items, MmrLeaf memory leaf, bytes memory digestScale) =
            abi.decode(proofBytes, (uint64, uint64, bytes32[], MmrLeaf, bytes));

        if (items.length >= 64) return false;

        bytes32 leafHash = hashLeaf(leaf);
        bytes32 root = mmrProofRoot(leafHash, MmrProof({ leafIndex: leafIndex, leafCount: leafCount, items: items }));
        if (!knownMmrRoot[root]) return false;

        // forge-lint: disable-next-line(asm-keccak256)
        if (keccak256(digestScale) != leaf.digestHash) return false;
        if (!_digestHasSccpCommitment(digestScale, messageId)) return false;

        return true;
    }

    /// @notice Keccak256 of SCALE-encoded `sp_beefy::Commitment<u32>` (payload = "mh" -> H256).
    function hashCommitment(Commitment calldata c) public pure returns (bytes32) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(_encodeCommitmentScale(c));
    }

    /// @notice Keccak256 of SCALE-encoded `sp_beefy::mmr::MmrLeaf<...LeafExtraData...>`.
    function hashLeaf(MmrLeaf memory leaf) public pure returns (bytes32) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(_encodeLeafScale(leaf));
    }

    /// @notice Compute the MMR root obtained by applying a simplified proof to `leafHash`.
    ///
    /// Proof format is aligned with Substrate `mmr::LeafProof` for a single leaf:
    /// - `leafIndex` (0-based)
    /// - `leafCount` (total leaves)
    /// - `items` (ordered proof items as produced by Substrate `mmr_generateProof`)
    ///
    /// Root construction follows `ckb-merkle-mountain-range` (used by Substrate MMR):
    /// - compute peak roots left->right (consuming proof items)
    /// - bag peaks right->left via `keccak256(right || left)`
    function mmrProofRoot(bytes32 leafHash, MmrProof memory proof) public pure returns (bytes32) {
        return _mmrRootFromProofSingle(leafHash, proof.leafIndex, proof.leafCount, proof.items);
    }

    function _mmrRootFromProofSingle(
        bytes32 leafHash,
        uint64 leafIndex,
        uint64 leafCount,
        bytes32[] memory proofItems
    ) internal pure returns (bytes32) {
        if (leafCount == 0) revert InvalidMMRProof();
        if (leafIndex >= leafCount) revert InvalidMMRProof();

        // Derive MMR size (#nodes) from the leaf count.
        uint64 mmrSize = _leafIndexToMmrSize(leafCount - 1);
        MmrRootCalcState memory st = MmrRootCalcState({
            leafPos: _leafIndexToPos(leafIndex),
            peaksLen: 0,
            proofIdx: 0,
            leafUsed: false
        });

        uint64[] memory peaks = _getPeaks(mmrSize);
        bytes32[] memory peaksHashes = new bytes32[](peaks.length + 1);
        for (uint256 i = 0; i < peaks.length; i++) {
            uint64 peakPos = peaks[i];
            if (!st.leafUsed && st.leafPos <= peakPos) {
                if (st.leafPos == peakPos) {
                    peaksHashes[st.peaksLen] = leafHash;
                } else {
                    (peaksHashes[st.peaksLen], st.proofIdx) =
                        _calculatePeakRootSingle(st.leafPos, leafHash, peakPos, proofItems, st.proofIdx);
                }
                st.leafUsed = true;
                st.peaksLen += 1;
            } else {
                // No leaf for this peak: proof carries the peak hash, or a bagged RHS peaks hash.
                if (st.proofIdx < proofItems.length) {
                    peaksHashes[st.peaksLen] = proofItems[st.proofIdx];
                    st.peaksLen += 1;
                    st.proofIdx += 1;
                } else {
                    break;
                }
            }
        }

        if (!st.leafUsed) revert InvalidMMRProof();

        // Optional bagged RHS peaks hash (see `ckb-merkle-mountain-range` proof generation).
        if (st.proofIdx < proofItems.length) {
            peaksHashes[st.peaksLen] = proofItems[st.proofIdx];
            st.peaksLen += 1;
            st.proofIdx += 1;
        }
        if (st.proofIdx != proofItems.length) revert InvalidMMRProof();
        if (st.peaksLen == 0) revert InvalidMMRProof();

        return _bagPeaks(peaksHashes, st.peaksLen);
    }

    function _bagPeaks(bytes32[] memory peaksHashes, uint256 peaksLen) internal pure returns (bytes32) {
        // Bag peaks right-to-left via hash(right, left).
        uint256 n = peaksLen;
        while (n > 1) {
            bytes32 right = peaksHashes[n - 1];
            bytes32 left = peaksHashes[n - 2];
            // forge-lint: disable-next-line(asm-keccak256)
            peaksHashes[n - 2] = keccak256(bytes.concat(right, left));
            n -= 1;
        }
        return peaksHashes[0];
    }

    function _calculatePeakRootSingle(
        uint64 pos,
        bytes32 item,
        uint64 peakPos,
        bytes32[] memory proofItems,
        uint256 proofIdx
    ) internal pure returns (bytes32 root, uint256 nextProofIdx) {
        uint32 height = 0;
        while (true) {
            if (pos == peakPos) {
                return (item, proofIdx);
            }

            uint32 nextHeight = _posHeightInTree(pos + 1);
            bool posIsRight = nextHeight > height;

            // Note: we do not need `sib_pos` for single-leaf verification, but we must follow the
            // same parent-position + merge-direction rules as the canonical implementation.
            uint64 parentPos = posIsRight ? (pos + 1) : (pos + _parentOffset(height));

            if (proofIdx >= proofItems.length) revert InvalidMMRProof();
            bytes32 sibling = proofItems[proofIdx++];

            // forge-lint: disable-next-line(asm-keccak256)
            bytes32 parentItem = posIsRight
                ? keccak256(bytes.concat(sibling, item))
                : keccak256(bytes.concat(item, sibling));

            if (parentPos > peakPos) revert InvalidMMRProof();

            pos = parentPos;
            item = parentItem;
            height += 1;
        }
        revert InvalidMMRProof();
    }

    // --- MMR helpers (ported from `ckb-merkle-mountain-range`) ---

    function _leafIndexToPos(uint64 index) internal pure returns (uint64) {
        uint64 mmrSize = _leafIndexToMmrSize(index);
        uint64 tz = _trailingZeros(index + 1);
        // mmr_size - tz - 1
        return mmrSize - tz - 1;
    }

    function _leafIndexToMmrSize(uint64 index) internal pure returns (uint64) {
        uint64 leavesCount = index + 1;
        uint64 peakCount = _popcount(leavesCount);
        // 2 * leaves_count - peak_count
        uint256 size = 2 * uint256(leavesCount) - uint256(peakCount);
        if (size > type(uint64).max) revert InvalidMMRProof();
        // forge-lint: disable-next-line(unsafe-typecast)
        return uint64(size);
    }

    function _posHeightInTree(uint64 pos) internal pure returns (uint32) {
        // ckb: pos += 1; while !all_ones(pos) { pos = jump_left(pos) }; height = bit_length(pos) - 1
        uint64 p = pos + 1;
        while (!_isAllOnes(p)) {
            uint64 msb = _highestPowerOfTwo(p);
            p = p - (msb - 1);
        }
        uint32 bl = _bitLength(p);
        if (bl == 0) revert InvalidMMRProof();
        return bl - 1;
    }

    function _parentOffset(uint32 height) internal pure returns (uint64) {
        return uint64(2) << height;
    }

    function _siblingOffset(uint32 height) internal pure returns (uint64) {
        return (uint64(2) << height) - 1;
    }

    function _getPeaks(uint64 mmrSize) internal pure returns (uint64[] memory) {
        if (mmrSize == 0) revert InvalidMMRProof();

        // Peak count is at most 64 for u64-sized MMR.
        uint64[] memory tmp = new uint64[](64);
        uint256 n = 0;

        (uint32 height, uint64 pos) = _leftPeakHeightPos(mmrSize);
        tmp[n++] = pos;
        while (height > 0) {
            (bool ok, uint32 h2, uint64 p2) = _getRightPeak(height, pos, mmrSize);
            if (!ok) break;
            height = h2;
            pos = p2;
            tmp[n++] = pos;
        }

        uint64[] memory out = new uint64[](n);
        for (uint256 i = 0; i < n; i++) {
            out[i] = tmp[i];
        }
        return out;
    }

    function _getRightPeak(
        uint32 height,
        uint64 pos,
        uint64 mmrSize
    ) internal pure returns (bool ok, uint32 outHeight, uint64 outPos) {
        uint64 p = pos + _siblingOffset(height);
        uint32 h = height;
        while (p > mmrSize - 1) {
            if (h == 0) {
                return (false, 0, 0);
            }
            p -= _parentOffset(h - 1);
            h -= 1;
        }
        return (true, h, p);
    }

    function _leftPeakHeightPos(uint64 mmrSize) internal pure returns (uint32, uint64) {
        uint32 height = 1;
        uint64 prevPos = 0;
        uint64 pos = _peakPosByHeight(height);
        while (pos < mmrSize) {
            height += 1;
            prevPos = pos;
            pos = _peakPosByHeight(height);
        }
        return (height - 1, prevPos);
    }

    function _peakPosByHeight(uint32 height) internal pure returns (uint64) {
        // (1 << (height + 1)) - 2
        if (height >= 63) revert InvalidMMRProof();
        return (uint64(1) << (height + 1)) - 2;
    }

    function _popcount(uint64 x) internal pure returns (uint64) {
        uint64 c = 0;
        uint64 v = x;
        while (v != 0) {
            v &= (v - 1);
            c += 1;
        }
        return c;
    }

    function _trailingZeros(uint64 x) internal pure returns (uint64) {
        if (x == 0) revert InvalidMMRProof();
        uint64 c = 0;
        uint64 v = x;
        while ((v & 1) == 0) {
            c += 1;
            v >>= 1;
        }
        return c;
    }

    function _bitLength(uint64 x) internal pure returns (uint32) {
        uint32 l = 0;
        uint64 v = x;
        while (v != 0) {
            l += 1;
            v >>= 1;
        }
        return l;
    }

    function _isAllOnes(uint64 x) internal pure returns (bool) {
        // x is of form 2^k - 1
        return x != 0 && (x & (x + 1)) == 0;
    }

    function _highestPowerOfTwo(uint64 x) internal pure returns (uint64) {
        if (x == 0) revert InvalidMMRProof();
        uint64 p = 1;
        uint64 v = x;
        // Find msb via shifting.
        while (v > 1) {
            v >>= 1;
            p <<= 1;
        }
        return p;
    }

    function _addKnownMmrRoot(bytes32 root) internal {
        if (knownMmrRoot[root]) return;

        bytes32 evicted = mmrRoots[mmrRootsPos];
        if (evicted != bytes32(0)) {
            knownMmrRoot[evicted] = false;
        }

        mmrRoots[mmrRootsPos] = root;
        knownMmrRoot[root] = true;
        mmrRootsPos = (mmrRootsPos + 1) % MMR_ROOT_HISTORY_SIZE;
    }

    function _verifyCommitmentSignatures(
        Commitment calldata commitment,
        ValidatorProof calldata proof,
        ValidatorSet memory vset
    ) internal pure {
        uint32 num = vset.len;
        if (num == 0) revert InvalidValidatorProof();
        uint32 threshold = uint32((2 * uint256(num) + 2) / 3); // ceil(2 * num / 3)

        uint256 n = proof.signatures.length;
        if (
            proof.positions.length != n ||
            proof.publicKeys.length != n ||
            proof.publicKeyMerkleProofs.length != n
        ) revert InvalidValidatorProof();

        if (n < threshold) revert NotEnoughValidatorSignatures();

        // Ensure unique positions and unique validator keys (fail-closed on duplicates).
        bytes memory seen = new bytes((uint256(num) + 7) / 8);

        bytes32 commitmentHash = hashCommitment(commitment);
        for (uint256 i = 0; i < n; i++) {
            uint256 pos = proof.positions[i];
            address pubkey = proof.publicKeys[i];

            if (pos >= num) revert ValidatorSetIncorrectPosition();
            if (_bitIsSet(seen, pos)) revert InvalidValidatorProof();
            _bitSet(seen, pos);

            // O(n^2) uniqueness check; `n` is bounded by `vset.len` and caller pays gas.
            for (uint256 j = 0; j < i; j++) {
                if (proof.publicKeys[j] == pubkey) revert InvalidValidatorProof();
            }

            if (!_verifyValidatorInSet(vset.root, num, pos, pubkey, proof.publicKeyMerkleProofs[i])) {
                revert ValidatorSetIncorrectPosition();
            }

            if (!_verifySignature(commitmentHash, pubkey, proof.signatures[i])) revert InvalidSignature();
        }
    }

    function _verifySignature(bytes32 msgHash, address expected, bytes memory sig) internal pure returns (bool) {
        if (sig.length != 65) return false;
        bytes32 r;
        bytes32 s;
        uint8 v;
        // solhint-disable-next-line no-inline-assembly
        assembly {
            r := mload(add(sig, 32))
            s := mload(add(sig, 64))
            v := byte(0, mload(add(sig, 96)))
        }
        if (v < 27) v += 27;
        if (v != 27 && v != 28) return false;
        // Reject malleable / invalid ECDSA signatures (EIP-2 style).
        if (r == bytes32(0) || s == bytes32(0)) return false;
        if (uint256(s) > SECP256K1N_HALF_ORDER) return false;
        address recovered = ecrecover(msgHash, v, r, s);
        return recovered != address(0) && recovered == expected;
    }

    function _verifyValidatorInSet(
        bytes32 root,
        uint32 setLen,
        uint256 pos,
        address addr,
        bytes32[] memory proof
    ) internal pure returns (bool) {
        if (pos >= setLen) return false;

        // Substrate `binary_merkle_tree` (ordered, no sorting):
        // - leafHash = keccak256(leaf_bytes) where leaf_bytes = bytes20(address)
        // - internal: keccak256(left || right)
        // - if odd number of nodes: last node is promoted
        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 h = keccak256(bytes.concat(bytes20(addr)));

        uint256 idx = pos;
        uint256 n = uint256(setLen);
        uint256 used = 0;
        while (n > 1) {
            bool isRight = (idx & 1) == 1;
            if (isRight) {
                if (used >= proof.length) return false;
                bytes32 sibling = proof[used++];
                // forge-lint: disable-next-line(asm-keccak256)
                h = keccak256(bytes.concat(sibling, h));
            } else {
                // If this is the last odd node, it is promoted without hashing.
                if (idx != n - 1) {
                    if (used >= proof.length) return false;
                    bytes32 sibling = proof[used++];
                    // forge-lint: disable-next-line(asm-keccak256)
                    h = keccak256(bytes.concat(h, sibling));
                }
            }
            idx = idx >> 1;
            n = (n + 1) >> 1;
        }
        return used == proof.length && h == root;
    }

    function _digestHasSccpCommitment(bytes memory digestScale, bytes32 messageId) internal pure returns (bool) {
        (uint256 n, uint256 off, bool ok) = _readCompactU32(digestScale, 0);
        if (!ok) return false;

        uint256 found = 0;
        for (uint256 i = 0; i < n; i++) {
            if (off >= digestScale.length) return false;
            uint8 itemKind = uint8(digestScale[off]);
            off += 1;
            if (itemKind != AUX_DIGEST_ITEM_COMMITMENT) return false;

            if (off >= digestScale.length) return false;
            uint8 networkKind = uint8(digestScale[off]);
            off += 1;

            uint256 networkId;
            if (networkKind == GENERIC_NETWORK_ID_EVM_LEGACY) {
                if (off + 4 > digestScale.length) return false;
                networkId = uint256(uint32(uint8(digestScale[off])) |
                    (uint32(uint8(digestScale[off + 1])) << 8) |
                    (uint32(uint8(digestScale[off + 2])) << 16) |
                    (uint32(uint8(digestScale[off + 3])) << 24));
                off += 4;
            } else if (networkKind == GENERIC_NETWORK_ID_EVM) {
                // EVMChainId = H256 (32 bytes)
                if (off + 32 > digestScale.length) return false;
                off += 32;
                networkId = type(uint256).max; // not SCCP
            } else if (networkKind == GENERIC_NETWORK_ID_SUB) {
                // SubNetworkId enum (1 byte)
                if (off + 1 > digestScale.length) return false;
                off += 1;
                networkId = type(uint256).max;
            } else if (networkKind == GENERIC_NETWORK_ID_TON) {
                // TonNetworkId enum (1 byte)
                if (off + 1 > digestScale.length) return false;
                off += 1;
                networkId = type(uint256).max;
            } else {
                return false;
            }

            if (off + 32 > digestScale.length) return false;
            bytes32 commitmentHash;
            // solhint-disable-next-line no-inline-assembly
            assembly {
                commitmentHash := mload(add(add(digestScale, 32), off))
            }
            off += 32;

            if (networkId == SCCP_DIGEST_NETWORK_ID && commitmentHash == messageId) {
                found += 1;
            }
        }

        // SCALE vectors must be consumed exactly; reject trailing bytes.
        return found == 1 && off == digestScale.length;
    }

    function _readCompactU32(bytes memory data, uint256 off) internal pure returns (uint256 v, uint256 newOff, bool ok) {
        if (off >= data.length) return (0, off, false);
        uint8 b0 = uint8(data[off]);
        uint8 mode = b0 & 0x03;
        if (mode == 0) {
            return (uint256(b0 >> 2), off + 1, true);
        }
        if (mode == 1) {
            if (off + 2 > data.length) return (0, off, false);
            uint16 b1 = uint16(uint8(data[off + 1]));
            v = uint256(uint16(b0) >> 2) | (uint256(b1) << 6);
            return (v, off + 2, true);
        }
        if (mode == 2) {
            if (off + 4 > data.length) return (0, off, false);
            v =
                (uint256(b0) >> 2) |
                (uint256(uint8(data[off + 1])) << 6) |
                (uint256(uint8(data[off + 2])) << 14) |
                (uint256(uint8(data[off + 3])) << 22);
            return (v, off + 4, true);
        }
        // mode == 3 (big int) not supported here (fail-closed).
        return (0, off, false);
    }

    function _encodeCommitmentScale(Commitment calldata c) internal pure returns (bytes memory out) {
        // SCALE(Payload(Vec<(BeefyPayloadId, Vec<u8>)>)) where payload is exactly one entry:
        // "mh" -> SCALE(H256 mmrRoot) (32 bytes).
        //
        // Commitment encoding: payload || u32 blockNumber (LE) || u64 validatorSetId (LE).
        out = new bytes(48);
        out[0] = 0x04; // compact vec len = 1
        out[1] = 0x6d; // 'm'
        out[2] = 0x68; // 'h'
        out[3] = 0x80; // compact vec<u8> len = 32
        // mmrRoot at offset 4
        bytes32 mmrRoot = c.mmrRoot;
        // solhint-disable-next-line no-inline-assembly
        assembly {
            mstore(add(add(out, 32), 4), mmrRoot)
        }
        _writeLe32(out, 36, c.blockNumber);
        _writeLe64(out, 40, c.validatorSetId);
    }

    function _encodeLeafScale(MmrLeaf memory leaf) internal pure returns (bytes memory out) {
        out = new bytes(145);
        out[0] = bytes1(leaf.version);
        _writeLe32(out, 1, leaf.parentNumber);
        bytes32 parentHash = leaf.parentHash;
        bytes32 nextAuthoritySetRoot = leaf.nextAuthoritySetRoot;
        bytes32 randomSeed = leaf.randomSeed;
        bytes32 digestHash = leaf.digestHash;
        // parentHash at offset 5 (un-aligned store is OK; memory is byte-addressed)
        // solhint-disable-next-line no-inline-assembly
        assembly {
            mstore(add(add(out, 32), 5), parentHash)
        }
        _writeLe64(out, 37, leaf.nextAuthoritySetId);
        _writeLe32(out, 45, leaf.nextAuthoritySetLen);
        // nextAuthoritySetRoot at offset 49
        // solhint-disable-next-line no-inline-assembly
        assembly {
            mstore(add(add(out, 32), 49), nextAuthoritySetRoot)
        }
        // randomSeed at offset 81
        // solhint-disable-next-line no-inline-assembly
        assembly {
            mstore(add(add(out, 32), 81), randomSeed)
        }
        // digestHash at offset 113
        // solhint-disable-next-line no-inline-assembly
        assembly {
            mstore(add(add(out, 32), 113), digestHash)
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

    function _bitIsSet(bytes memory bf, uint256 index) private pure returns (bool) {
        uint256 byteIndex = index >> 3;
        uint256 bitInByte = index & 7;
        uint8 mask = _msb0Mask(bitInByte);
        return (uint8(bf[byteIndex]) & mask) != 0;
    }

    function _bitSet(bytes memory bf, uint256 index) private pure {
        uint256 byteIndex = index >> 3;
        uint256 bitInByte = index & 7;
        uint8 mask = _msb0Mask(bitInByte);
        bf[byteIndex] = bytes1(uint8(bf[byteIndex]) | mask);
    }

    function _msb0Mask(uint256 bitInByte) private pure returns (uint8) {
        if (bitInByte == 0) return 0x80;
        if (bitInByte == 1) return 0x40;
        if (bitInByte == 2) return 0x20;
        if (bitInByte == 3) return 0x10;
        if (bitInByte == 4) return 0x08;
        if (bitInByte == 5) return 0x04;
        if (bitInByte == 6) return 0x02;
        return 0x01;
    }
}
