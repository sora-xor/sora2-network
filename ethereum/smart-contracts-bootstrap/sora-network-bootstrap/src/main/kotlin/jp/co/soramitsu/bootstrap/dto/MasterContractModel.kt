/*
 * Copyright D3 Ledger, Inc. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

package jp.co.soramitsu.bootstrap.dto

import java.math.BigInteger
import javax.validation.constraints.NotNull

data class SigsData(
    val vv: ArrayList<BigInteger>,
    val rr: ArrayList<ByteArray>,
    val ss: ArrayList<ByteArray>
)

data class UpdateMasterContractResponse(
    val success: Boolean = false
) : Conflictable() {
    constructor(errorCode: String? = null, message: String? = null) :
            this(false) {
        this.errorCode = errorCode
        this.message = message
    }
}

data class UpdateBridgeContractRequest(
    @NotNull val network: EthereumNetworkProperties = EthereumNetworkProperties(),
    @NotNull val bridgeContract: BridgeContractProperties = BridgeContractProperties(),
    val newPeerAddress: String? = null,
    val removePeerAddress: String? = null
)

data class BridgeContractProperties(
    @NotNull val address: String? = null,
    @NotNull val peers: List<InitWalletInfo> = emptyList()
)

data class InitWalletInfo(
    @NotNull val password: String = "",
    @NotNull val path: String = ""
)

data class DeploySORABridgeRequest(
    @NotNull val network: EthereumNetworkProperties = EthereumNetworkProperties(),
    @NotNull val peerAccounts: List<String> = emptyList(),
    @NotNull val networkId: BigInteger,
    val addressXOR: String,
    val addressVAL: String
)

data class DeploySORANFTMigrationRequest(
    @NotNull val network: EthereumNetworkProperties = EthereumNetworkProperties(),
    @NotNull val peerAccounts: List<String> = emptyList()
)

data class DeploySORAContractResponse(
    val contractAddress: String? = null,
    val soraAddress: String? = null
) : Conflictable()

