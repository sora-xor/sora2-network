// SPDX-License-Identifier: BSD-4-Clause
//
// Gas + fee estimation helpers for Jetton wallet operations.
//
// This file is adapted from `ton-blockchain/tolk-bench` Jetton contracts (MIT),
// with SCCP-specific burn message sizing tweaks.

import "@stdlib/gas-payments"
import "errors"

fun getPrecompiledGasConsumption(): int?
    asm "GETPRECOMPILEDGAS"

// Storage costs
const STORAGE_SIZE_MaxWallet_bits         = 1033
const STORAGE_SIZE_MaxWallet_cells        = 3
const STORAGE_SIZE_InitStateWallet_bits   = 931
const STORAGE_SIZE_InitStateWallet_cells  = 3

// Message sizes (rough upper bounds).
const MESSAGE_SIZE_BurnNotification_bits  = 754
const MESSAGE_SIZE_BurnNotification_cells = 1

// SCCP burn adds destDomain (32 bits) + recipient32 (256 bits).
const MESSAGE_SIZE_SccpBurnNotification_bits  = 1200
const MESSAGE_SIZE_SccpBurnNotification_cells = 1

const MIN_STORAGE_DURATION = 5 * 365 * 24 * 3600  // 5 years

// Gas costs (best-effort estimates; overestimation is safer than underestimation).
const GAS_CONSUMPTION_JettonTransfer    = 9000
const GAS_CONSUMPTION_JettonReceive     = 10000
const GAS_CONSUMPTION_BurnRequest       = 7000
const GAS_CONSUMPTION_BurnNotification  = 7000

fun calculateJettonWalletMinStorageFee() {
    return calculateStorageFee(BASECHAIN, MIN_STORAGE_DURATION, STORAGE_SIZE_MaxWallet_bits, STORAGE_SIZE_MaxWallet_cells);
}

fun forwardInitStateOverhead() {
    return calculateForwardFeeWithoutLumpPrice(BASECHAIN, STORAGE_SIZE_InitStateWallet_bits, STORAGE_SIZE_InitStateWallet_cells);
}

fun checkAmountIsEnoughToTransfer(msgValue: int, forwardTonAmount: int, fwdFee: int) {
    var fwdCount = forwardTonAmount ? 2 : 1;

    var jettonWalletGasConsumption = getPrecompiledGasConsumption();
    var sendTransferGasConsumption = (jettonWalletGasConsumption == null) ? GAS_CONSUMPTION_JettonTransfer : jettonWalletGasConsumption;
    var receiveTransferGasConsumption = (jettonWalletGasConsumption == null) ? GAS_CONSUMPTION_JettonReceive : jettonWalletGasConsumption;

    assert (msgValue >
        forwardTonAmount +
        fwdCount * fwdFee +
        forwardInitStateOverhead() +
        calculateGasFee(BASECHAIN, sendTransferGasConsumption) +
        calculateGasFee(BASECHAIN, receiveTransferGasConsumption) +
        calculateJettonWalletMinStorageFee()
    ) throw ERROR_NOT_ENOUGH_GAS;
}

fun checkAmountIsEnoughToBurn(msgValue: int) {
    var jettonWalletGasConsumption = getPrecompiledGasConsumption();
    var sendBurnGasConsumption = (jettonWalletGasConsumption == null) ? GAS_CONSUMPTION_BurnRequest : jettonWalletGasConsumption;

    assert (msgValue >
        calculateForwardFee(BASECHAIN, MESSAGE_SIZE_BurnNotification_bits, MESSAGE_SIZE_BurnNotification_cells) +
        calculateGasFee(BASECHAIN, sendBurnGasConsumption) +
        calculateGasFee(BASECHAIN, GAS_CONSUMPTION_BurnNotification)
    ) throw ERROR_NOT_ENOUGH_GAS;
}

fun checkAmountIsEnoughToSccpBurn(msgValue: int) {
    var jettonWalletGasConsumption = getPrecompiledGasConsumption();
    var sendBurnGasConsumption = (jettonWalletGasConsumption == null) ? GAS_CONSUMPTION_BurnRequest : jettonWalletGasConsumption;

    assert (msgValue >
        calculateForwardFee(BASECHAIN, MESSAGE_SIZE_SccpBurnNotification_bits, MESSAGE_SIZE_SccpBurnNotification_cells) +
        calculateGasFee(BASECHAIN, sendBurnGasConsumption) +
        calculateGasFee(BASECHAIN, GAS_CONSUMPTION_BurnNotification)
    ) throw ERROR_NOT_ENOUGH_GAS;
}
