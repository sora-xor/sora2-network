// SPDX-License-Identifier: BSD-4-Clause
//
// SCCP Jetton storage types.

// SnakeString describes a (potentially long) string inside a cell.
type SnakeString = slice

fun SnakeString.unpackFromSlice(mutate s: slice) {
    // SnakeString is always "the remainder" in our use-cases.
    val snakeRemainder = s;
    s = createEmptySlice();
    return snakeRemainder
}

fun SnakeString.packToBuilder(self, mutate b: builder) {
    b.storeSlice(self)
}

struct WalletStorage {
    status: uint4
    jettonBalance: coins
    ownerAddress: address
    minterAddress: address
}

struct SccpBurnRecord {
    burnInitiator: address
    destDomain: uint32
    recipient32: uint256
    jettonAmount: coins
    nonce: uint64
}

struct SccpStorageExtra {
    soraAssetId: uint256
    nonce: uint64
    inboundPausedMask: uint64
    outboundPausedMask: uint64

    // Incident controls.
    invalidatedInbound: map<uint256, bool>
    processedInbound: map<uint256, bool>

    // On-chain burn records keyed by SCCP messageId.
    burns: map<uint256, Cell<SccpBurnRecord>>
}

struct MinterStorage {
    totalSupply: coins

    // Jetton admin / SCCP governor (governance control).
    governorAddress: address

    // SCCP verifier contract allowed to trigger mints (fail-closed until set).
    verifierAddress: address?

    // Standard Jetton config.
    jettonWalletCode: cell
    metadataUri: Cell<SnakeString>

    // SCCP fields are kept in a ref to avoid exceeding the 1023-bit cell limit.
    sccp: Cell<SccpStorageExtra>
}

fun MinterStorage.load() {
    return MinterStorage.fromCell(contract.getData())
}

fun MinterStorage.save(self) {
    contract.setData(self.toCell())
}

fun WalletStorage.load() {
    return WalletStorage.fromCell(contract.getData())
}

fun WalletStorage.save(self) {
    contract.setData(self.toCell())
}
