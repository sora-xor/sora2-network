// SPDX-License-Identifier: BSD-4-Clause
//
// Jetton helper functions (wallet address derivation / deploy StateInit).

import "storage"

// We work in basechain. SCCP recipient encoding assumes workchain 0.
const MY_WORKCHAIN = BASECHAIN

fun calcDeployedJettonWallet(ownerAddress: address, minterAddress: address, jettonWalletCode: cell): AutoDeployAddress {
    val emptyWalletStorage: WalletStorage = {
        status: 0,
        jettonBalance: 0,
        ownerAddress,
        minterAddress,
    };

    return {
        workchain: MY_WORKCHAIN,
        stateInit: {
            code: jettonWalletCode,
            data: emptyWalletStorage.toCell()
        }
    }
}

fun calcAddressOfJettonWallet(ownerAddress: address, minterAddress: address, jettonWalletCode: cell) {
    val jwDeployed = calcDeployedJettonWallet(ownerAddress, minterAddress, jettonWalletCode);
    return jwDeployed.calculateAddress()
}

