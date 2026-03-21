// SPDX-License-Identifier: BSD-4-Clause
//
// Trustless SCCP on-chain verifier for TON that verifies SORA burns using a BEEFY+MMR light client.
//
// Model (same as EVM/Solana verifier repos):
// - Anyone can perform the one-time bootstrap for initial validator sets + latest BEEFY block.
// - Anyone can import finalized SORA MMR roots by submitting:
//   - a BEEFY commitment (payload = MMR root, block number, validator set id)
//   - >=2/3 validator signatures + merkle membership proofs against the stored validator set root
//   - an MMR leaf + simplified inclusion proof under that MMR root (also used for validator set rotation)
// - Once an MMR root is imported, anyone can mint on TON by presenting a burn proof under a known root:
//   - messageId must appear exactly once in the SCALE-encoded SORA auxiliary digest as
//     AuxiliaryDigestItem::Commitment(GenericNetworkId::EVMLegacy('SCCP'), messageId)
//
// TVM features used:
// - `HASHEXT_KECCAK256` for keccak256 (including multi-slice hashing via `keccak256Concat2`)
// - `ECRECOVER` for secp256k1 public key recovery from Ethereum-style (v,r,s) signatures

import "@stdlib/gas-payments"
import "errors"
import "messages"
import "sccp-codec"

const MMR_ROOT_HISTORY_SIZE = 30
const SECP256K1_HALF_ORDER = 0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0

struct ValidatorSet {
    id: uint64
    len: uint32
    root: uint256
}

struct LightClientState {
    initialized: bool
    latestBeefyBlock: uint64

    currentValidatorSet: ValidatorSet
    nextValidatorSet: ValidatorSet

    mmrRootsPos: uint16
    mmrRoots: map<uint16, uint256>
    knownRoots: map<uint256, bool>
}

struct VerifierStorage {
    governorAddress: address
    jettonMasterAddress: address
    soraAssetId: uint256
    state: Cell<LightClientState>
}

fun VerifierStorage.load() {
    return VerifierStorage.fromCell(contract.getData())
}

fun VerifierStorage.save(self) {
    contract.setData(self.toCell())
}

type AllowedMessageToVerifier =
    | SccpVerifierSetGovernor
    | SccpVerifierInitialize
    | SccpVerifierSubmitSignatureCommitment
    | SccpVerifierMintFromSoraProof
    | SccpVerifierMintFromSoraProofV2
    | TopUpTons

// Proof encoding (project-specific, shared with tests):
// proof cell bits:
// - leafIndex: uint64
// - leafCount: uint64
// - itemsRef: ref cell (u16 totalCount in first 16 bits, then uint256 items, snake refs for overflow)
// - leafRef: ref cell:
//   - leafVersion: uint8
//   - parentNumber: uint32
//   - parentHash: uint256
//   - nextAuthoritySetId: uint64
//   - nextAuthoritySetLen: uint32
//   - nextAuthoritySetRoot: uint256
//   - randomSeed: uint256
//   - digestScaleRef: ref cell (bytes; must be 8-bit aligned)

fun keccak256TwoU256(a: uint256, b: uint256): uint256 {
    val c = beginCell().storeUint(a, 256).storeUint(b, 256).endCell();
    return keccak256(c.beginParse());
}

// --- MMR helpers (ported from `ckb-merkle-mountain-range`) ---

fun popcount64(x: uint64): uint64 {
    var v = x;
    var c = 0 as uint64;
    while (v != 0) {
        v = v & (v - 1);
        c += 1;
    }
    return c;
}

fun trailingZeros64(x: uint64): uint64 {
    assert (x != 0) throw ERROR_SCCP_INVALID_MMR_PROOF;
    var v = x;
    var c = 0 as uint64;
    while ((v & 1) == 0) {
        v = v / 2;
        c += 1;
    }
    return c;
}

fun leafIndexToMmrSize(index: uint64): uint64 {
    val leavesCount = index + 1;
    val peakCount = popcount64(leavesCount);
    return 2 * leavesCount - peakCount;
}

fun leafIndexToPos(index: uint64): uint64 {
    val mmrSize = leafIndexToMmrSize(index);
    val tz = trailingZeros64(index + 1);
    return mmrSize - tz - 1;
}

fun isAllOnes(x: uint64): bool {
    return x != 0 && ((x & (x + 1)) == 0);
}

fun highestPowerOfTwo64(x: uint64): uint64 {
    assert (x != 0) throw ERROR_SCCP_INVALID_MMR_PROOF;
    var p = 1 as uint64;
    var v = x;
    while (v > 1) {
        v = v / 2;
        p *= 2;
    }
    return p;
}

fun bitLength64(x: uint64): uint32 {
    var l = 0 as uint32;
    var v = x;
    while (v != 0) {
        v = v / 2;
        l += 1;
    }
    return l;
}

fun posHeightInTree(pos: uint64): uint32 {
    // ckb: pos += 1; while !all_ones(pos) { pos = jump_left(pos) }; height = bit_length(pos) - 1
    var p = pos + 1;
    while (!isAllOnes(p)) {
        val msb = highestPowerOfTwo64(p);
        p = p - (msb - 1);
    }
    val bl = bitLength64(p);
    assert (bl > 0) throw ERROR_SCCP_INVALID_MMR_PROOF;
    return bl - 1;
}

fun parentOffset(height: uint32): uint64 {
    // 2 << height
    var v = 2 as uint64;
    var h = height;
    while (h > 0) {
        v *= 2;
        h -= 1;
    }
    return v;
}

fun siblingOffset(height: uint32): uint64 {
    return parentOffset(height) - 1;
}

fun peakPosByHeight(height: uint32): uint64 {
    // (1 << (height + 1)) - 2
    var v = 1 as uint64;
    var h = height + 1;
    while (h > 0) {
        v *= 2;
        h -= 1;
    }
    return v - 2;
}

fun leftPeakHeightPos(mmrSize: uint64): (uint32, uint64) {
    var height = 1 as uint32;
    var prevPos = 0 as uint64;
    var pos = peakPosByHeight(height);
    while (pos < mmrSize) {
        height += 1;
        prevPos = pos;
        pos = peakPosByHeight(height);
    }
    return (height - 1, prevPos);
}

fun getRightPeak(height: uint32, pos: uint64, mmrSize: uint64): (bool, uint32, uint64) {
    var h = height;
    var p = pos + siblingOffset(h);
    while (p > (mmrSize - 1)) {
        if (h == 0) {
            return (false, 0, 0);
        }
        p -= parentOffset(h - 1);
        h -= 1;
    }
    return (true, h, p);
}

fun loadNextProofItem(total: int, mutate idx: int, mutate itemsSlice: slice): uint256 {
    assert (idx < total) throw ERROR_SCCP_INVALID_MMR_PROOF;
    // Load sibling item, traversing snake refs if needed.
    if (itemsSlice.remainingBitsCount() < 256) {
        val maybeNext = itemsSlice.loadMaybeRef();
        assert (maybeNext != null) throw ERROR_SCCP_INVALID_MMR_PROOF;
        itemsSlice = maybeNext!.beginParse();
    }
    val out = itemsSlice.loadUint(256);
    idx += 1;
    return out;
}

fun calculatePeakRootSingle(
    leafPos: uint64,
    leafHash: uint256,
    peakPos: uint64,
    total: int,
    mutate proofIdx: int,
    mutate itemsSlice: slice,
): uint256 {
    var pos = leafPos;
    var item = leafHash;
    var height = 0 as uint32;

    while (true) {
        if (pos == peakPos) {
            return item;
        }

        val nextHeight = posHeightInTree(pos + 1);
        val posIsRight = nextHeight > height;
        val parentPos = posIsRight ? (pos + 1) : (pos + parentOffset(height));

        val sibling = loadNextProofItem(total, mutate proofIdx, mutate itemsSlice);
        val parentItem = posIsRight ? keccak256TwoU256(sibling, item) : keccak256TwoU256(item, sibling);
        assert (parentPos <= peakPos) throw ERROR_SCCP_INVALID_MMR_PROOF;

        pos = parentPos;
        item = parentItem;
        height += 1;
    }
}

fun readU8(mutate s: slice): int {
    return s.loadUint(8);
}

fun readU32LE(mutate s: slice): uint32 {
    // little-endian: b0 + (b1<<8) + (b2<<16) + (b3<<24)
    val b0 = s.loadUint(8);
    val b1 = s.loadUint(8);
    val b2 = s.loadUint(8);
    val b3 = s.loadUint(8);
    return b0 + b1 * 256 + b2 * 65536 + b3 * 16777216;
}

fun readCompactU32(mutate s: slice): (uint32, bool) {
    if (s.remainingBitsCount() < 8) {
        return (0, false);
    }
    val b0 = s.loadUint(8);
    val mode = b0 % 4;
    if (mode == 0) {
        return (b0 / 4, true);
    }
    if (mode == 1) {
        if (s.remainingBitsCount() < 8) {
            return (0, false);
        }
        val b1 = s.loadUint(8);
        return ((b0 / 4) + b1 * 64, true);
    }
    if (mode == 2) {
        if (s.remainingBitsCount() < 24) {
            return (0, false);
        }
        val b1 = s.loadUint(8);
        val b2 = s.loadUint(8);
        val b3 = s.loadUint(8);
        // (b0>>2) | (b1<<6) | (b2<<14) | (b3<<22)
        return ((b0 / 4) + b1 * 64 + b2 * 16384 + b3 * 4194304, true);
    }
    // mode 3 (big int) unsupported
    return (0, false);
}

fun digestHasSccpCommitment(digestScale: slice, messageId: uint256): bool {
    var s = digestScale;
    val (n, ok) = readCompactU32(mutate s);
    if (!ok) {
        return false;
    }

    var found = 0;
    var i = 0;
    while (i < n) {
        if (s.remainingBitsCount() < 16) {
            return false;
        }
        val itemKind = readU8(mutate s);
        if (itemKind != 0) { // AuxiliaryDigestItem::Commitment
            return false;
        }
        val networkKind = readU8(mutate s);

        var networkId = 0xffffffff;
        if (networkKind == 2) {
            // GenericNetworkId::EVMLegacy(u32 LE)
            if (s.remainingBitsCount() < 32) {
                return false;
            }
            networkId = readU32LE(mutate s);
        } else if (networkKind == 0) {
            // GenericNetworkId::EVM(H256) - skip 32 bytes
            if (s.remainingBitsCount() < 256) {
                return false;
            }
            s.skipBits(256);
        } else if (networkKind == 1) {
            // GenericNetworkId::Sub(enum u8) - skip 1 byte
            if (s.remainingBitsCount() < 8) {
                return false;
            }
            s.skipBits(8);
        } else if (networkKind == 3) {
            // GenericNetworkId::TON(enum u8) - skip 1 byte
            if (s.remainingBitsCount() < 8) {
                return false;
            }
            s.skipBits(8);
        } else {
            return false;
        }

        if (s.remainingBitsCount() < 256) {
            return false;
        }
        val commitmentHash = s.loadUint(256);
        if (networkId == SCCP_DIGEST_NETWORK_ID && commitmentHash == messageId) {
            found += 1;
        }

        i += 1;
    }
    // SCALE vectors must be consumed exactly; reject trailing bits or refs.
    if (found != 1 || s.remainingBitsCount() != 0 || s.remainingRefsCount() != 0) {
        return false;
    }
    return true;
}

fun addKnownMmrRoot(mutate st: LightClientState, root: uint256) {
    if (st.knownRoots.get(root).isFound) {
        return;
    }

    val pos = st.mmrRootsPos % MMR_ROOT_HISTORY_SIZE;
    val evicted = st.mmrRoots.get(pos);
    if (evicted.isFound) {
        st.knownRoots.delete(evicted.loadValue());
    }

    st.mmrRoots.set(pos, root);
    st.knownRoots.set(root, true);
    st.mmrRootsPos = (pos + 1) % MMR_ROOT_HISTORY_SIZE;
}

// --- ECDSA (secp256k1) helpers ---

@pure
fun ecrecoverFull(hash: uint256, v: int, r: uint256, s: uint256): (int?, uint256?, uint256?, bool)
    // ECRECOVER: `hash v r s - 0` or `h x1 x2 -1`
    // Normalize to 4 stack items:
    // - on success: h x1 x2 -1
    // - on failure: null null null 0
    asm(hash v r s) "ECRECOVER" "NULLSWAPIFNOT2" "NULLSWAPIFNOT"

fun ethAddressFromPubkeyXY(x: uint256, y: uint256): uint256 {
    val b = beginCell().storeUint(x, 256).storeUint(y, 256).endCell();
    val h = keccak256(b.beginParse());
    return h % SCCP_EVM_ADDRESS_MOD_U160;
}

fun loadNextU256FromSnake(mutate s: slice): (uint256, bool) {
    if (s.remainingBitsCount() < 256) {
        val maybeNext = s.loadMaybeRef();
        if (maybeNext == null) {
            return (0, false);
        }
        s = maybeNext!.beginParse();
    }
    if (s.remainingBitsCount() < 256) {
        return (0, false);
    }
    return (s.loadUint(256), true);
}

fun verifyBeefyMerkleProof(root: uint256, setLen: uint32, pos: uint32, addr160: uint256, proof: cell): bool {
    if (pos >= setLen) {
        return false;
    }
    // Leaf: keccak256(bytes20(addr)).
    val leafCell = beginCell().storeUint(addr160, 160).endCell();
    var current = keccak256(leafCell.beginParse());

    var s = proof.beginParse();
    val total = s.loadUint(16);
    assert (total <= 63) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;

    // Substrate `binary_merkle_tree` (ordered, no sorting):
    // - leafHash = keccak256(leaf_bytes) where leaf_bytes = bytes20(address)
    // - internal: keccak256(left || right)
    // - if odd number of nodes: last node is promoted
    var used = 0;
    var idx = pos;
    var n = setLen;
    while (n > 1) {
        val isRight = (idx % 2) == 1;
        if (isRight) {
            if (used >= total) {
                return false;
            }
            val (sib, ok) = loadNextU256FromSnake(mutate s);
            if (!ok) {
                return false;
            }
            current = keccak256TwoU256(sib, current);
            used += 1;
        } else {
            // If this is the last odd node, it is promoted without hashing.
            if (idx != (n - 1)) {
                if (used >= total) {
                    return false;
                }
                val (sib, ok) = loadNextU256FromSnake(mutate s);
                if (!ok) {
                    return false;
                }
                current = keccak256TwoU256(current, sib);
                used += 1;
            }
        }
        idx = idx / 2;
        n = (n + 1) / 2;
    }

    if (used != total || s.remainingBitsCount() != 0 || s.remainingRefsCount() != 0) {
        return false;
    }
    return current == root;
}

fun hashCommitment(mmrRoot: uint256, blockNumber: uint32, validatorSetId: uint64): uint256 {
    // SCALE(sp_beefy::Commitment<u32>) with payload restricted to one entry:
    // "mh" -> Vec<u8> of length 32 (mmr_root bytes).
    //
    // Layout (48 bytes):
    // [0] compact(vec len=1)=0x04
    // [1..3] "mh"
    // [3] compact(vec<u8> len=32)=0x80
    // [4..36] mmr_root
    // [36..40] u32 block_number (LE)
    // [40..48] u64 validator_set_id (LE)
    var b = beginCell();
    b.storeUint(0x04, 8);
    b.storeUint(0x6d, 8); // 'm'
    b.storeUint(0x68, 8); // 'h'
    b.storeUint(0x80, 8);
    b.storeUint(mmrRoot, 256);
    storeU32LE(mutate b, blockNumber);
    storeU64LE(mutate b, validatorSetId);
    return keccak256(b.endCell().beginParse());
}

fun verifyCommitmentSignatures(commitmentHash: uint256, vset: ValidatorSet, validatorProof: cell) {
    val num = vset.len;
    assert (num > 0) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;

    // >=2/3
    val threshold = num - ((num - 1) / 3);

    // Validator proof encoding (project-specific, shared with tests):
    // - validatorProof cell:
    //   - n: uint16 (number of signatures)
    //   - head: ref (linked list of `n` entries)
    // - each entry cell (must be sorted by `pos` strictly ascending):
    //   - v: uint8 (recovery id, typically 27/28 or 0/1)
    //   - r: uint256
    //   - s: uint256
    //   - pos: uint32 (leaf index in validator set)
    //   - merkleProof: ref (u16 count + sibling hashes, snake-ref encoded)
    //   - next: maybeRef (next entry)
    var s = validatorProof.beginParse();
    val n = s.loadUint(16);
    assert (n <= num) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;
    assert (n >= threshold) throw ERROR_SCCP_NOT_ENOUGH_VALIDATOR_SIGNATURES;

    // Proof entries must be sorted by validator position (strict ascending), so we can enforce
    // uniqueness without storing an in-memory set.
    var prevPos = -1;
    // Defense in depth: reject duplicate signer addresses even when position proofs are valid.
    // This prevents threshold inflation if an initialized validator set accidentally contains duplicates.
    var seenAddrs = createEmptyMap<uint256, bool>();

    var entry: cell? = s.loadRef();
    assert (s.remainingBitsCount() == 0 && s.remainingRefsCount() == 0) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;
    var i = 0;
    while (i < n) {
        assert (entry != null) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;
        var es = entry!.beginParse();
        var v = es.loadUint(8);
        val r = es.loadUint(256);
        val sigS = es.loadUint(256);
        val pos = es.loadUint(32);
        val merkleProof = es.loadRef();
        val next = es.loadMaybeRef();
        assert (es.remainingBitsCount() == 0 && es.remainingRefsCount() == 0) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;

        // Ethereum signatures commonly use v in {27, 28}; after normalization,
        // enforce EVM parity-only recovery id in {0,1}.
        if (v >= 27) {
            v -= 27;
        }
        assert (v <= 1) throw ERROR_SCCP_INVALID_SIGNATURE;
        // Reject malleable / invalid signatures (EIP-2 style).
        assert (r != 0 && sigS != 0) throw ERROR_SCCP_INVALID_SIGNATURE;
        assert (sigS <= SECP256K1_HALF_ORDER) throw ERROR_SCCP_INVALID_SIGNATURE;

        val (_hOpt, xOpt, yOpt, ok) = ecrecoverFull(commitmentHash, v, r, sigS);
        assert (ok) throw ERROR_SCCP_INVALID_SIGNATURE;

        val addr = ethAddressFromPubkeyXY(xOpt!, yOpt!);
        assert (pos < num) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;
        assert (pos > prevPos) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;
        assert (!seenAddrs.get(addr).isFound) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;
        seenAddrs.set(addr, true);
        prevPos = pos;

        assert (verifyBeefyMerkleProof(vset.root, num, pos, addr, merkleProof)) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;

        entry = next;
        i += 1;
    }

    // No extra signatures allowed.
    assert (entry == null) throw ERROR_SCCP_INVALID_VALIDATOR_PROOF;
}

fun computeProofRootAndNextSet(proof: cell): (uint256, slice, uint64, uint32, uint256) {
    // Returns (root, digestScaleSlice, nextAuthoritySetId, nextAuthoritySetLen, nextAuthoritySetRoot).
    // Throws on malformed proof.
    var s = proof.beginParse();
    val leafIndex = s.loadUint(64);
    val leafCount = s.loadUint(64);
    val itemsRef = s.loadRef();
    // Leaf fields do not fit into a single cell together with (leafIndex, leafCount),
    // so they are stored in a separate ref cell.
    val leafRef = s.loadRef();
    assert (s.remainingBitsCount() == 0 && s.remainingRefsCount() == 0) throw ERROR_SCCP_INVALID_MMR_PROOF;
    var ls = leafRef.beginParse();

    val leafVersion = ls.loadUint(8);
    val parentNumber = ls.loadUint(32);
    val parentHash = ls.loadUint(256);
    val nextAuthoritySetId = ls.loadUint(64);
    val nextAuthoritySetLen = ls.loadUint(32);
    val nextAuthoritySetRoot = ls.loadUint(256);
    val randomSeed = ls.loadUint(256);
    val digestScaleRef = ls.loadRef();
    assert (ls.remainingBitsCount() == 0 && ls.remainingRefsCount() == 0) throw ERROR_SCCP_INVALID_MMR_PROOF;

    // Digest hash binding.
    // `keccak256(slice)` may consume the slice cursor, so we parse twice:
    // - once for hashing (digestHash in the leaf encoding)
    // - once for parsing commitments
    val digestScaleForHash = digestScaleRef.beginParse();
    val digestHash = keccak256(digestScaleForHash);
    val digestScaleSlice = digestScaleRef.beginParse();

    // SCALE encoding of leaf (145 bytes) does not fit into a single TON cell (1023 bits max),
    // so we split it into two slices and use `HASHEXT_KECCAK256` with `n = 2`.
    //
    // leaf bytes:
    // - prefix (113 bytes): version + parentNumber(le) + parentHash + nextAuthSetId(le) + nextAuthSetLen(le) + nextAuthSetRoot + randomSeed
    // - suffix (32 bytes): digestHash
    var b1 = beginCell();
    b1.storeUint(leafVersion, 8);
    storeU32LE(mutate b1, parentNumber);
    b1.storeUint(parentHash, 256);
    storeU64LE(mutate b1, nextAuthoritySetId);
    storeU32LE(mutate b1, nextAuthoritySetLen);
    b1.storeUint(nextAuthoritySetRoot, 256);
    b1.storeUint(randomSeed, 256);

    var b2 = beginCell();
    b2.storeUint(digestHash, 256);

    val leafHash = keccak256Concat2(b1.endCell().beginParse(), b2.endCell().beginParse());

    // Compute MMR root from a Substrate-style single-leaf proof (ckb-merkle-mountain-range).
    var itemsSlice = itemsRef.beginParse();
    val total = itemsSlice.loadUint(16);
    assert (total <= 63) throw ERROR_SCCP_INVALID_MMR_PROOF;
    assert (leafCount > 0) throw ERROR_SCCP_INVALID_MMR_PROOF;
    assert (leafIndex < leafCount) throw ERROR_SCCP_INVALID_MMR_PROOF;

    val mmrSize = leafIndexToMmrSize((leafCount - 1) as uint64);
    val leafPos = leafIndexToPos(leafIndex as uint64);

    val (peakHeight0, peakPos0) = leftPeakHeightPos(mmrSize);
    var peakHeight = peakHeight0;
    var peakPos = peakPos0;

    var peaksHashes = createEmptyMap<uint16, uint256>();
    var peaksLen = 0 as uint16;
    var proofIdx = 0;
    var leafUsed = false;

    var keepIteratingPeaks = true;
    while (keepIteratingPeaks) {
        // Process current peak.
        if (!leafUsed && leafPos <= peakPos) {
            val peakRoot = (leafPos == peakPos)
                ? leafHash
                : calculatePeakRootSingle(leafPos, leafHash, peakPos, total, mutate proofIdx, mutate itemsSlice);
            leafUsed = true;
            peaksHashes.set(peaksLen, peakRoot);
            peaksLen += 1;
        } else {
            // No leaf for this peak: proof carries the peak hash, or a bagged RHS peaks hash.
            if (proofIdx < total) {
                val peakRoot = loadNextProofItem(total, mutate proofIdx, mutate itemsSlice);
                peaksHashes.set(peaksLen, peakRoot);
                peaksLen += 1;
            } else {
                keepIteratingPeaks = false;
            }
        }

        if (keepIteratingPeaks) {
            if (peakHeight == 0) {
                keepIteratingPeaks = false;
            } else {
                val (ok, h2, p2) = getRightPeak(peakHeight, peakPos, mmrSize);
                if (!ok) {
                    keepIteratingPeaks = false;
                } else {
                    peakHeight = h2;
                    peakPos = p2;
                }
            }
        }
    }

    assert (leafUsed) throw ERROR_SCCP_INVALID_MMR_PROOF;

    // Optional bagged RHS peaks hash.
    if (proofIdx < total) {
        val rhs = loadNextProofItem(total, mutate proofIdx, mutate itemsSlice);
        peaksHashes.set(peaksLen, rhs);
        peaksLen += 1;
    }
    assert (proofIdx == total) throw ERROR_SCCP_INVALID_MMR_PROOF;
    assert (itemsSlice.remainingBitsCount() == 0 && itemsSlice.remainingRefsCount() == 0) throw ERROR_SCCP_INVALID_MMR_PROOF;
    assert (peaksLen > 0) throw ERROR_SCCP_INVALID_MMR_PROOF;

    // Bag peaks right-to-left via hash(right, left).
    while (peaksLen > 1) {
        val right = peaksHashes.get(peaksLen - 1);
        val left = peaksHashes.get(peaksLen - 2);
        assert (right.isFound && left.isFound) throw ERROR_SCCP_INVALID_MMR_PROOF;
        val bagged = keccak256TwoU256(right.loadValue(), left.loadValue());
        peaksHashes.set(peaksLen - 2, bagged);
        peaksLen -= 1;
    }
    val root = peaksHashes.get(0);
    assert (root.isFound) throw ERROR_SCCP_INVALID_MMR_PROOF;

    return (root.loadValue(), digestScaleSlice, nextAuthoritySetId, nextAuthoritySetLen, nextAuthoritySetRoot);
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessageToVerifier.fromSlice(in.body);

    match (msg) {
        SccpVerifierSetGovernor => {
            throw ERROR_SCCP_ADMIN_PATH_DISABLED;
        }

        SccpVerifierInitialize => {
            var storage = lazy VerifierStorage.load();

            var st = lazy storage.state.load();
            assert (!st.initialized) throw ERROR_SCCP_VERIFIER_ALREADY_INITIALIZED;

            st.initialized = true;
            st.latestBeefyBlock = msg.latestBeefyBlock;
            st.currentValidatorSet = ValidatorSet {
                id: msg.currentValidatorSetId,
                len: msg.currentValidatorSetLen,
                root: msg.currentValidatorSetRoot,
            };
            st.nextValidatorSet = ValidatorSet {
                id: msg.nextValidatorSetId,
                len: msg.nextValidatorSetLen,
                root: msg.nextValidatorSetRoot,
            };

            st.mmrRootsPos = 0;
            st.mmrRoots = createEmptyMap<uint16, uint256>();
            st.knownRoots = createEmptyMap<uint256, bool>();

            storage.state = st.toCell();
            storage.save();

            val registerMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: storage.jettonMasterAddress,
                value: 0,
                body: SccpSetVerifier {
                    queryId: msg.queryId,
                    newVerifier: contract.getAddress(),
                }
            });
            registerMsg.send(SEND_MODE_IGNORE_ERRORS | SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
        }

        SccpVerifierSubmitSignatureCommitment => {
            var storage = lazy VerifierStorage.load();
            var st = lazy storage.state.load();
            assert (st.initialized) throw ERROR_SCCP_VERIFIER_NOT_INITIALIZED;

            val blockNo64 = (msg.commitmentBlockNumber as uint64);
            assert (blockNo64 > st.latestBeefyBlock) throw ERROR_SCCP_COMMITMENT_TOO_OLD;

            var vset = st.currentValidatorSet;
            if (msg.commitmentValidatorSetId == st.nextValidatorSet.id) {
                vset = st.nextValidatorSet;
            } else if (msg.commitmentValidatorSetId != st.currentValidatorSet.id) {
                throw ERROR_SCCP_INVALID_VALIDATOR_SET_ID;
            }

            val cHash = hashCommitment(
                msg.commitmentMmrRoot,
                msg.commitmentBlockNumber,
                msg.commitmentValidatorSetId,
            );
            verifyCommitmentSignatures(cHash, vset, msg.validatorProof);

            val (root, _digestScale, nextId, nextLen, nextRoot) = computeProofRootAndNextSet(msg.latestLeafProof);
            assert (root == msg.commitmentMmrRoot) throw ERROR_SCCP_INVALID_MMR_PROOF;

            addKnownMmrRoot(mutate st, root);
            st.latestBeefyBlock = blockNo64;

            // Apply validator set changes (if any) from the leaf.
            if (nextId > st.nextValidatorSet.id) {
                st.currentValidatorSet = st.nextValidatorSet;
                st.nextValidatorSet = ValidatorSet {
                    id: nextId,
                    len: nextLen,
                    root: nextRoot,
                };
            }

            storage.state = st.toCell();
            storage.save();
        }

        SccpVerifierMintFromSoraProof => {
            var storage = lazy VerifierStorage.load();
            val st = lazy storage.state.load();
            assert (st.initialized) throw ERROR_SCCP_VERIFIER_NOT_INITIALIZED;

            // Compute canonical SCCP messageId for SORA -> TON (dest domain fixed to TON).
            val messageId = sccpBurnMessageIdV1(
                SCCP_DOMAIN_SORA,
                SCCP_DOMAIN_TON,
                msg.burnNonce,
                storage.soraAssetId,
                msg.jettonAmount,
                msg.recipient32,
            );

            val (root, digestScale, _nextId, _nextLen, _nextRoot) = computeProofRootAndNextSet(msg.proof);

            assert (st.knownRoots.get(root).isFound) throw ERROR_SCCP_UNKNOWN_MMR_ROOT;
            assert (digestHasSccpCommitment(digestScale, messageId)) throw ERROR_SCCP_COMMITMENT_NOT_FOUND;

            // Forward mint to jetton master.
            val mintMsg = createMessage({
                bounce: BounceMode.RichBounce,
                dest: storage.jettonMasterAddress,
                value: 0,
                body: SccpMintFromVerifier {
                    queryId: msg.queryId,
                    sourceDomain: SCCP_DOMAIN_SORA,
                    burnNonce: msg.burnNonce,
                    jettonAmount: msg.jettonAmount,
                    recipient32: msg.recipient32,
                    sendExcessesTo: msg.sendExcessesTo,
                }
            });
            mintMsg.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
        }

        SccpVerifierMintFromSoraProofV2 => {
            var storage = lazy VerifierStorage.load();
            val st = lazy storage.state.load();
            assert (st.initialized) throw ERROR_SCCP_VERIFIER_NOT_INITIALIZED;
            sccpEnsureSupportedDomain(msg.sourceDomain);
            // V2 is for non-TON sources attested by SORA into BEEFY+MMR.
            assert (msg.sourceDomain != SCCP_DOMAIN_TON) throw ERROR_SCCP_DOMAIN_UNSUPPORTED;

            // Compute canonical SCCP messageId for `sourceDomain` -> TON (dest domain fixed to TON).
            // This supports SCCP burns from any source chain, as long as SORA committed that `messageId`
            // into its auxiliary digest (BEEFY+MMR finalized).
            val messageId = sccpBurnMessageIdV1(
                msg.sourceDomain,
                SCCP_DOMAIN_TON,
                msg.burnNonce,
                storage.soraAssetId,
                msg.jettonAmount,
                msg.recipient32,
            );

            val (root, digestScale, _nextId, _nextLen, _nextRoot) = computeProofRootAndNextSet(msg.proof);

            assert (st.knownRoots.get(root).isFound) throw ERROR_SCCP_UNKNOWN_MMR_ROOT;
            assert (digestHasSccpCommitment(digestScale, messageId)) throw ERROR_SCCP_COMMITMENT_NOT_FOUND;

            // Forward mint to jetton master.
            val mintMsg = createMessage({
                bounce: BounceMode.RichBounce,
                dest: storage.jettonMasterAddress,
                value: 0,
                body: SccpMintFromVerifier {
                    queryId: msg.queryId,
                    sourceDomain: msg.sourceDomain,
                    burnNonce: msg.burnNonce,
                    jettonAmount: msg.jettonAmount,
                    recipient32: msg.recipient32,
                    sendExcessesTo: msg.sendExcessesTo,
                }
            });
            mintMsg.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
        }

        TopUpTons => {
            // just accept tons
        }

        else => throw ERROR_WRONG_OP
    }
}

// --- Getters ---

get fun get_verifier_config(): (address, address, uint256, bool, uint64, uint64, uint64) {
    val storage = lazy VerifierStorage.load();
    val st = lazy storage.state.load();
    return (
        storage.governorAddress,
        storage.jettonMasterAddress,
        storage.soraAssetId,
        st.initialized,
        st.latestBeefyBlock,
        st.currentValidatorSet.id,
        st.nextValidatorSet.id
    );
}

get fun is_known_root(root: uint256): bool {
    val storage = lazy VerifierStorage.load();
    val st = lazy storage.state.load();
    return st.knownRoots.get(root).isFound;
}
