/*
 * Copyright D3 Ledger, Inc. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

package jp.co.soramitsu.bootstrap.controller

import jp.co.soramitsu.bootstrap.dto.*
import jp.co.soramitsu.bootstrap.utils.DeployHelper
import jp.co.soramitsu.bootstrap.utils.DeployHelperBuilder
import jp.co.soramitsu.bootstrap.sidechain.util.hashToAddAndRemovePeer
import jp.co.soramitsu.bootstrap.utils.defaultByteHash
import jp.co.soramitsu.bootstrap.utils.defaultIrohaHash
import jp.co.soramitsu.bootstrap.utils.prepareSignatures
import mu.KLogging
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.*
import org.web3j.crypto.Keys
import org.web3j.crypto.Wallet
import org.web3j.crypto.WalletFile
import org.web3j.crypto.WalletUtils
import org.web3j.crypto.WalletUtils.loadJsonCredentials
import javax.validation.constraints.NotNull

@RestController
@RequestMapping("/eth")
class EthController {

    @PostMapping("/deploy/bridge/update")
    fun addPeerToBridgeContract(@NotNull @RequestBody request: UpdateBridgeContractRequest): ResponseEntity<UpdateMasterContractResponse> {
        try {
            val deployHelper = createSmartContractDeployHelper(request.network)
            if (request.bridgeContract.address == null) {
                return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                    .body(UpdateMasterContractResponse(HttpStatus.BAD_REQUEST.name))
            }
            val bridge = deployHelper.loadBridgeContract(request.bridgeContract.address)
            val ecKeyPairs = request.bridgeContract.peers.map {
                WalletUtils.loadCredentials(
                    it.password,
                    it.path
                )
            }.map { it.ecKeyPair }

            if (ecKeyPairs.isEmpty()) {
                throw IllegalArgumentException(
                    "Provide paths to wallets of notaries, " +
                            "registered in smart contract for signature creation"
                )
            }

            var addResult = true
            var removeResult = true
            var errorStatus: String? = null
            var errorMessage: String? = null

            if (request.removePeerAddress != null) {
                val finalHash = prepareTrxHash(request.removePeerAddress)
                val sigs = prepareSignatures(
                    request.bridgeContract.peers.size,
                    ecKeyPairs,
                    finalHash
                )
                val trxResult = bridge.removePeerByPeer(
                    request.removePeerAddress,
                    defaultByteHash,
                    sigs.vv,
                    sigs.rr,
                    sigs.ss
                ).send()
                if (!trxResult.isStatusOK) {
                    errorStatus = trxResult.status
                    errorMessage =
                        "Error removeAddress action. Transaction hash is: trxResult.transactionHash"
                    removeResult = trxResult.isStatusOK
                }
            }
            if (request.newPeerAddress != null) {
                val finalHash = prepareTrxHash(request.newPeerAddress)
                val sigs = prepareSignatures(
                    request.bridgeContract.peers.size,
                    ecKeyPairs,
                    finalHash
                )

                val trxResult = bridge.addPeerByPeer(
                    request.newPeerAddress,
                    defaultByteHash,
                    sigs.vv,
                    sigs.rr,
                    sigs.ss
                ).send()
                if (!trxResult.isStatusOK) {
                    errorStatus = trxResult.status
                    errorMessage =
                        "Error addAddress action. Transaction hash is: trxResult.transactionHash"
                    addResult = trxResult.isStatusOK
                }
            }
            val response = UpdateMasterContractResponse(addResult && removeResult)
            response.message = errorMessage
            response.errorCode = errorStatus
            return if (addResult && removeResult)
                ResponseEntity.ok(response)
            else
                ResponseEntity.status(HttpStatus.CONFLICT).body(response)
        } catch (e: Exception) {
            logger.error("Error adding peer to smart contract", e)
            return ResponseEntity.status(HttpStatus.CONFLICT).body(
                UpdateMasterContractResponse(e.javaClass.simpleName, e.message)
            )
        }
    }


    @PostMapping("/deploy/{project}/bridge")
    fun deployBridgeDeployerSmartContract(@PathVariable("project") project: String, @NotNull @RequestBody request: DeploySORABridgeRequest): ResponseEntity<DeploySORAContractResponse> {
        val deployHelper = createSmartContractDeployHelper(request.network)
        return try {
            if (project != "sora2") {
                throw IllegalArgumentException("Wrong project name $project")
            }
            val bridgeDeployer = deployHelper.deployBridgeDeployerSmartContract(
                request.peerAccounts,
                request.addressVAL,
                request.addressXOR,
                request.networkId
            )
            val masterAddress = deployHelper.deployBridgeSmartContract(bridgeDeployer)
            ResponseEntity.ok(
                DeploySORAContractResponse(
                    masterAddress
                )
            )

        } catch (e: Exception) {
            logger.error("Cannot deploy Bridge smart contract", e)
            val response = DeploySORAContractResponse()
            response.errorCode = e.javaClass.simpleName
            response.message = e.message
            ResponseEntity.status(HttpStatus.CONFLICT).body(response)
        } finally {
            deployHelper.web3.shutdown()
        }
    }

    @PostMapping("/deploy/{project}/nftMigration")
    fun deployNFTMigrationSmartContract(@PathVariable("project") project: String, @NotNull @RequestBody request: DeploySORANFTMigrationRequest): ResponseEntity<DeploySORAContractResponse> {
        val deployHelper = createSmartContractDeployHelper(request.network)
        return try {
            if (project != "sora2") {
                throw IllegalArgumentException("Wrong project name $project")
            }
            val master = deployHelper.deployNftMigrationSmartContract(
                request.peerAccounts
            )
            ResponseEntity.ok(
                DeploySORAContractResponse(
                    master.contractAddress
                )
            )
        } catch (e: Exception) {
            logger.error("Cannot deploy NFT Migration smart contract", e)
            val response = DeploySORAContractResponse()
            response.errorCode = e.javaClass.simpleName
            response.message = e.message
            ResponseEntity.status(HttpStatus.CONFLICT).body(response)
        } finally {
            deployHelper.web3.shutdown()
        }
    }

    @GetMapping("/create/wallet")
    fun createWallet(@NotNull @RequestParam password: String): ResponseEntity<EthWallet> {
        return try {
            val wallet: WalletFile = Wallet.createStandard(password, Keys.createEcKeyPair())
            ResponseEntity.ok(EthWallet(wallet))
        } catch (e: Exception) {
            logger.error("Error creating Ethereum wallet", e)
            val response = EthWallet()
            response.errorCode = e.javaClass.simpleName
            response.message = e.message
            ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR).body(response)
        }
    }

    @GetMapping("/list/servicesWithWallet/{project}/{peersCount}")
    fun listServiceEthWallets(@PathVariable("project") project: String, @PathVariable("peersCount") peersCount: Int): ResponseEntity<EthWalletsList> {
        val list = ArrayList<String>()
        list.add("eth-genesis-wallet")
        repeat(peersCount) {
            list.add("eth-deposit-service-peer$it")
        }

        return ResponseEntity.ok(EthWalletsList(list))
    }

    private fun createSmartContractDeployHelper(network: EthereumNetworkProperties): DeployHelper {
        val credentials = loadJsonCredentials(
            network.ethereumCredentials.credentialsPassword,
            network.ethereumCredentials.credentials
        )
        return DeployHelperBuilder(
            network.ethereumConfig,
            network.ethereumCredentials.nodeLogin,
            network.ethereumCredentials.nodePassword,
            credentials
        )
            .setFastTransactionManager()
            .build()
    }

    private fun prepareTrxHash(removePeerAddress: String): String {
        return hashToAddAndRemovePeer(
            removePeerAddress,
            defaultIrohaHash
        )
    }

    companion object : KLogging()
}
