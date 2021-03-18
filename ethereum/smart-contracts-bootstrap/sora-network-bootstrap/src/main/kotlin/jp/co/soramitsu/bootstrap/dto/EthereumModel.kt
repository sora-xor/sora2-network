/*
 * Copyright D3 Ledger, Inc. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

package jp.co.soramitsu.bootstrap.dto

import jp.co.soramitsu.soranet.eth.config.EthereumConfig
import org.web3j.crypto.WalletFile

data class EthereumNetworkProperties(
    val ethereumCredentials: EthereumCredentials = EthereumCredentials(),
    val ethereumConfig: EthereumConfigImpl = EthereumConfigImpl()
)

/**
 * Credentials for Ethereum network
 * @param credentials - encrypted ethereum credentials in JSON
 * @param credentialsPassword - password to decrypt credentials
 * @param nodeLogin - login to Ethereum node
 * @param nodePassword - password to Ethereum node
 */
data class EthereumCredentials(
    val credentials: String = "",
    val credentialsPassword: String = "user",
    val nodeLogin: String? = null,
    val nodePassword: String? = null
)

/**
 * Default parameters are Ropsten testnet parameters
 */
data class EthereumConfigImpl(
    override val url: String = "http://parity-d3.test.iroha.tech:8545",
    override val gasPrice: Long = 100000000000,
    override val gasLimit: Long = 4500000,
    override val confirmationPeriod: Long = 0
) : EthereumConfig

data class EthWallet(val file: WalletFile? = null) : Conflictable()

/**
 * List of services that require Ethereum wallets
 */
data class EthWalletsList(val wallets: List<String>? = null) : Conflictable() {
    constructor(errorCode: String? = null, message: String? = null) :
            this() {
        this.errorCode = errorCode
        this.message = message
    }
}
