// SPDX-License-Identifier: BSD-4-Clause
//
// SCCP codec and messageId computation (BurnPayloadV1 SCALE encoding + keccak256).

import "errors"

const SCCP_DOMAIN_SORA = 0
const SCCP_DOMAIN_ETH = 1
const SCCP_DOMAIN_BSC = 2
const SCCP_DOMAIN_SOL = 3
const SCCP_DOMAIN_TON = 4
const SCCP_DOMAIN_TRON = 5

// Leaf provider digest commitment network id sentinel (matches SORA pallet `SCCP_DIGEST_NETWORK_ID`).
// 'SCCP' ASCII, big-endian (u32): 0x53 0x43 0x43 0x50.
const SCCP_DIGEST_NETWORK_ID = 0x53434350

const SCCP_MSG_PREFIX_BURN_V1 = "sccp:burn:v1"

const MAX_U64 = 0xffffffffffffffff
const MAX_U128 = 0xffffffffffffffffffffffffffffffff

// 2^160 (used to validate canonical EVM address encoding inside a 32-byte field).
const SCCP_EVM_ADDRESS_MOD_U160 = 0x10000000000000000000000000000000000000000

@pure
fun keccak256(s: slice): uint256
    // HASHEXT expects an extra integer argument (mode). `ONE` means "hash data bits".
    asm "ONE" "HASHEXT_KECCAK256"

// Computes keccak256(a || b) for two slices.
//
// NOTE: TVM cells can only hold up to 1023 bits of data, so for larger byte strings we must
// hash multiple slices with `HASHEXT_KECCAK256` by specifying `n = 2`.
@pure
fun keccak256Concat2(a: slice, b: slice): uint256
    asm(a b) "TWO" "HASHEXT_KECCAK256"

fun storeUintLE(mutate b: builder, value: int, bytes: int) {
    var v = value;
    var i = 0;
    while (i < bytes) {
        // store least significant byte first
        b.storeUint(v % 256, 8);
        v = v / 256;
        i += 1;
    }
}

fun storeU32LE(mutate b: builder, value: uint32) {
    storeUintLE(mutate b, value, 4);
}

fun storeU64LE(mutate b: builder, value: uint64) {
    storeUintLE(mutate b, value, 8);
}

fun storeU128LE(mutate b: builder, value: coins) {
    assert (value >= 0 && value <= MAX_U128) throw ERROR_SCCP_AMOUNT_TOO_LARGE;
    storeUintLE(mutate b, value, 16);
}

fun sccpBurnMessageIdV1(
    sourceDomain: uint32,
    destDomain: uint32,
    nonce: uint64,
    soraAssetId: uint256,
    amount: coins,
    recipient32: uint256,
): uint256 {
    // BurnPayloadV1 SCALE bytes:
    // version:u8 | source_domain:u32(le) | dest_domain:u32(le) | nonce:u64(le) | sora_asset_id:[32] | amount:u128(le) | recipient:[32]
    var b = beginCell();
    b.storeSlice(SCCP_MSG_PREFIX_BURN_V1);

    // payload
    b.storeUint(1, 8);
    storeU32LE(mutate b, sourceDomain);
    storeU32LE(mutate b, destDomain);
    storeU64LE(mutate b, nonce);
    b.storeUint(soraAssetId, 256);
    storeU128LE(mutate b, amount);
    b.storeUint(recipient32, 256);

    // hashext expects a slice, not a cell.
    return keccak256(b.endCell().beginParse());
}

@pure
fun sccpIsRecipientZero(recipient32: uint256): bool {
    return recipient32 == 0;
}

@pure
fun sccpIsEvmDomain(domain: uint32): bool {
    return domain == SCCP_DOMAIN_ETH || domain == SCCP_DOMAIN_BSC || domain == SCCP_DOMAIN_TRON;
}

@pure
fun sccpIsSupportedDomain(domain: uint32): bool {
    return domain == SCCP_DOMAIN_SORA
        || domain == SCCP_DOMAIN_ETH
        || domain == SCCP_DOMAIN_BSC
        || domain == SCCP_DOMAIN_SOL
        || domain == SCCP_DOMAIN_TON
        || domain == SCCP_DOMAIN_TRON;
}

fun sccpEnsureSupportedDomain(domain: uint32) {
    assert (sccpIsSupportedDomain(domain)) throw ERROR_SCCP_DOMAIN_UNSUPPORTED;
}

@pure
fun sccpIsCanonicalEvmRecipient(recipient32: uint256): bool {
    // EVM recipient encoding: 20-byte address right-aligned in a 32-byte field.
    // Require high 12 bytes to be zero and require the 20-byte address to be non-zero.
    return (recipient32 / SCCP_EVM_ADDRESS_MOD_U160) == 0
        && (recipient32 % SCCP_EVM_ADDRESS_MOD_U160) != 0;
}

@pure
fun sccpMaskHasDomain(mask: uint64, domain: uint32): bool {
    // mask bit i corresponds to domain i.
    // Use arithmetic to avoid relying on bitops in older toolchains.
    var d = domain;
    var div = mask;
    while (d > 0) {
        div = div / 2;
        d -= 1;
    }
    return (div % 2) == 1;
}

fun sccpMaskSetDomainPaused(mask: uint64, domain: uint32, paused: bool): uint64 {
    assert (sccpIsSupportedDomain(domain)) throw ERROR_SCCP_DOMAIN_UNSUPPORTED;
    if (sccpMaskHasDomain(mask, domain) == paused) {
        return mask;
    }
    // compute 2^domain
    var bit = 1;
    var i = 0;
    while (i < domain) {
        bit *= 2;
        i += 1;
    }
    return paused ? (mask + bit) : (mask - bit);
}
