/*
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

package jp.co.soramitsu.bootstrap.utils

import com.d3.commons.util.createPrettyScheduledThreadPool
import jp.co.soramitsu.soranet.eth.config.EthereumConfig
import jp.co.soramitsu.soranet.eth.config.EthereumPasswords
import jp.co.soramitsu.soranet.eth.contract.Bridge
import jp.co.soramitsu.soranet.eth.contract.BridgeDeployer
import jp.co.soramitsu.soranet.eth.contract.BridgeDeployerEVM
import jp.co.soramitsu.soranet.eth.contract.NftMigration
import mu.KLogging
import okhttp3.*
import org.web3j.contracts.eip20.generated.ERC20
import org.web3j.crypto.WalletUtils
import org.web3j.protocol.Web3j
import org.web3j.protocol.core.DefaultBlockParameterName
import org.web3j.protocol.core.JsonRpc2_0Web3j.DEFAULT_BLOCK_TIME
import org.web3j.protocol.core.RemoteCall
import org.web3j.protocol.core.RemoteFunctionCall
import org.web3j.protocol.core.methods.response.TransactionReceipt
import org.web3j.protocol.http.HttpService
import org.web3j.tx.Contract
import org.web3j.tx.RawTransactionManager
import org.web3j.tx.gas.DefaultGasProvider
import org.web3j.tx.gas.StaticGasProvider
import java.io.IOException
import java.math.BigInteger
import java.util.*
import java.util.concurrent.TimeUnit


const val ATTEMPTS_DEFAULT = 240

/**
 * Authenticator class for basic access authentication
 * @param nodePassword config with Ethereum node credentials
 */
class BasicAuthenticator(private val nodeLogin: String?, private val nodePassword: String?) :
    Authenticator {
    constructor(ethereumPasswords: EthereumPasswords) : this(
        ethereumPasswords.nodeLogin,
        ethereumPasswords.nodePassword
    )

    override fun authenticate(route: Route?, response: Response): Request {
        val credential = Credentials.basic(nodeLogin!!, nodePassword!!)
        return response.request.newBuilder().header("Authorization", credential).build()
    }
}

/**
 * Build DeployHelper in more granular level
 * @param ethereumConfig config with Ethereum network parameters
 * @param nodeLogin - Ethereum node login
 * @param nodePassword - Ethereum node password
 * @param credentials - Ethereum credentials
 */
class DeployHelperBuilder(
    ethereumConfig: EthereumConfig,
    nodeLogin: String?,
    nodePassword: String?,
    val credentials: org.web3j.crypto.Credentials,
    private val attempts: Int = ATTEMPTS_DEFAULT
) {
    /**
     * Helper class for contracts deploying
     * @param ethereumConfig config with Ethereum network parameters
     * @param ethereumPasswords config with Ethereum passwords
     */
    constructor(ethereumConfig: EthereumConfig, ethereumPasswords: EthereumPasswords) :
            this(
                ethereumConfig,
                ethereumPasswords.nodeLogin,
                ethereumPasswords.nodePassword,
                WalletUtils.loadCredentials(
                    ethereumPasswords.credentialsPassword,
                    ethereumPasswords.credentialsPath
                ),
                ATTEMPTS_DEFAULT
            )

    private val deployHelper =
        DeployHelper(ethereumConfig, nodeLogin, nodePassword, credentials, attempts)

    /**
     * Specify fast transaction manager to send multiple transactions one by one.
     */
    fun setFastTransactionManager(): DeployHelperBuilder {
        deployHelper.defaultTransactionManager = AttemptsCustomizableFastRawTransactionManager(
            deployHelper.web3,
            credentials,
            attempts
        )
        return this
    }

    fun build(): DeployHelper {
        return deployHelper
    }
}

/**
 * Helper class for contracts deploying
 * @param ethereumConfig config with Ethereum network parameters
 * @param nodeLogin - Ethereum node login
 * @param nodePassword
 * @param attempts attempts amount to poll transaction status
 */
class DeployHelper(
    ethereumConfig: EthereumConfig,
    nodeLogin: String?,
    nodePassword: String?,
    val credentials: org.web3j.crypto.Credentials,
    attempts: Int = ATTEMPTS_DEFAULT
) {
    /**
     * Helper class for contracts deploying
     * @param ethereumConfig config with Ethereum network parameters
     * @param ethereumPasswords config with Ethereum passwords
     */
    constructor(ethereumConfig: EthereumConfig, ethereumPasswords: EthereumPasswords) :
            this(
                ethereumConfig,
                ethereumPasswords.nodeLogin,
                ethereumPasswords.nodePassword,
                WalletUtils.loadCredentials(
                    ethereumPasswords.credentialsPassword,
                    ethereumPasswords.credentialsPath
                ),
                ATTEMPTS_DEFAULT
            )

    val web3: Web3j

    init {
        val builder = OkHttpClient().newBuilder()
        builder.authenticator(BasicAuthenticator(nodeLogin, nodePassword))
        builder.readTimeout(1200, TimeUnit.SECONDS)
        builder.writeTimeout(1200, TimeUnit.SECONDS)
        web3 = Web3j.build(
            HttpService(ethereumConfig.url, builder.build(), false), DEFAULT_BLOCK_TIME.toLong(),
            createPrettyScheduledThreadPool(DeployHelper::class.simpleName!!, "web3j")
        )
    }

    /** transaction manager */
    var defaultTransactionManager = RawTransactionManager(web3, credentials, attempts, DEFAULT_BLOCK_TIME)

    /** Gas price */
    val gasPrice = BigInteger.valueOf(ethereumConfig.gasPrice)

    /** Max gas limit */
    val gasLimit = BigInteger.valueOf(ethereumConfig.gasLimit)

    /**
     * Deploy bridge deployer smart contract
     * @return bridge deployer contract object
     */
    fun deployBridgeDeployerSmartContract(
        peers: List<String>,
        addressVAL: String,
        addressXOR: String,
        networkId: BigInteger
    ): Contract {
        val bridgeDeployerContract: RemoteCall<out Contract>
        if (BigInteger.ZERO == networkId) {
            bridgeDeployerContract = BridgeDeployer.deploy(
                web3,
                credentials,
                StaticGasProvider(gasPrice, gasLimit),
                peers,
                addressVAL,
                addressXOR,
                Arrays.copyOfRange(BigInteger(networkId.toString(), 16).toByteArray(), 1, 33)
            )
        } else {
            bridgeDeployerContract = BridgeDeployerEVM.deploy(
                web3,
                credentials,
                StaticGasProvider(gasPrice, gasLimit),
                peers,
                Arrays.copyOfRange(BigInteger(networkId.toString(), 16).toByteArray(), 1, 33)
            )
        }

        val contract = bridgeDeployerContract.send()
        logger.info { "Bridge deployer smart contract ${contract.contractAddress} was deployed" }
        return contract
    }

    /**
     * Deploy bridge smart contract
     * @return bridge smart contract object
     */
    fun deployBridgeSmartContract(
        deployer: Contract
    ): String {
        val bridgeContract: RemoteFunctionCall<TransactionReceipt>
        val transactionReceipt = TransactionReceipt()
        val bridgeAddressFunction: (TransactionReceipt) -> String
        when (deployer) {
            is BridgeDeployer -> {
                logger.info { "Deploying Ethereum bridge ..." }
                bridgeContract = deployer.deployBridgeContract()
                bridgeAddressFunction = { deployer.getNewBridgeDeployedEvents(transactionReceipt)[0].bridgeAddress }
            }
            is BridgeDeployerEVM -> {
                logger.info("Deploying Generic EVM bridge")
                bridgeContract = deployer.deployBridgeContract()
                bridgeAddressFunction = { deployer.getNewBridgeDeployedEVMEvents(transactionReceipt)[0].bridgeAddress }
            }
            else -> {
                throw RuntimeException("Unsupported type of network ${deployer.javaClass.name}");
            }
        }
        val bridge = bridgeContract.send()
        logger.info { "Bridge smart contract transaction hash ${bridge.transactionHash}" }
        transactionReceipt.logs = bridge.logs
        val contractAddress = bridgeAddressFunction.invoke(transactionReceipt)
        logger.info { "Bridge smart contract address is ${contractAddress}" }
        return contractAddress
    }

    fun deployNftMigrationSmartContract(
        peers: List<String>
    ): NftMigration {
        val nftMigration = NftMigration.deploy(
            web3,
            credentials,
            StaticGasProvider(gasPrice, gasLimit),
            peers
        ).send()
        logger.info { "NFTMigration smart contract ${nftMigration.contractAddress} was deployed" }
        return nftMigration
    }


    /**
     * Load Master contract implementation
     * @param address - address of master contract
     * @return Master contract
     */
    fun loadBridgeContract(address: String): Bridge {
        return Bridge.load(
            address,
            web3,
            defaultTransactionManager,
            StaticGasProvider(gasPrice, gasLimit)
        )
    }

    /**
     * Load Master contract implementation
     * @param address - address of master contract
     * @return Master contract
     */
    fun loadNftMigrationContract(address: String): NftMigration {
        return NftMigration.load(
            address,
            web3,
            defaultTransactionManager,
            StaticGasProvider(gasPrice, gasLimit)
        )
    }




    /**
     * Send ERC20 tokens
     * @param tokenAddress - address of token smart contract
     * @param toAddress - address transfer to
     * @param amount - amount of tokens
     * @param transactionManager - transaction manager to use
     */
    fun sendERC20(
        tokenAddress: String,
        toAddress: String,
        amount: BigInteger,
        transactionManager: RawTransactionManager = defaultTransactionManager
    ) {
        val token = ERC20.load(
            tokenAddress,
            web3,
            transactionManager,
            StaticGasProvider(gasPrice, gasLimit)
        )
        token.transfer(toAddress, amount).send()
        logger.info { "ERC20 $amount with address $tokenAddress were sent to $toAddress" }
    }

    /**
     * Send ERC20 tokens
     * @param tokenAddress - address of token smart contract
     * @param toAddress - address transfer to
     * @param amount - amount of tokens
     * @param amount - credentials to use
     */
    fun sendERC20(
        tokenAddress: String,
        toAddress: String,
        amount: BigInteger,
        credentials: org.web3j.crypto.Credentials
    ) {
        return sendERC20(
            tokenAddress,
            toAddress,
            amount,
            RawTransactionManager(
                web3,
                credentials,
                ATTEMPTS_DEFAULT,
                DEFAULT_BLOCK_TIME
            )
        )
    }

    /**
     * Get ERC20 balance
     * @param tokenAddress - address of token smart contract
     * @param whoAddress - user address to check
     * @return user balance
     */
    fun getERC20Balance(tokenAddress: String, whoAddress: String): BigInteger {
        val token = ERC20.load(
            tokenAddress,
            web3,
            defaultTransactionManager,
            StaticGasProvider(gasPrice, gasLimit)
        )
        return token.balanceOf(whoAddress).send()
    }

    /**
     * Get ETH balance
     * @param whoAddress - user address to check
     * @return user balance
     */
    fun getETHBalance(whoAddress: String): BigInteger {
        return web3.ethGetBalance(whoAddress, DefaultBlockParameterName.LATEST).send().balance
    }

    /**
     * Signs user-provided data with predefined account deployed on local Parity node
     * @param toSign data to sign
     * @return signed data
     */
    fun signUserData(toSign: String) =
        jp.co.soramitsu.bootstrap.sidechain.util.signUserData(credentials.ecKeyPair, toSign)

    /**
     * Logger
     */
    companion object : KLogging() {
        private val defaultGasProvider = DefaultGasProvider()
    }
}

/**
 * Simple RawTransactionManager derivative that manages nonces to facilitate multiple transactions
 * per block. The implementation allows to set the attempts amount to modify default timeout.
 */
class AttemptsCustomizableFastRawTransactionManager(
    web3j: Web3j,
    credentials: org.web3j.crypto.Credentials,
    attempts: Int
) : RawTransactionManager(
    web3j,
    credentials,
    attempts,
    DEFAULT_BLOCK_TIME
) {

    @Volatile
    var currentNonce = BigInteger.valueOf(-1)!!
        private set

    @Synchronized
    @Throws(IOException::class)
    override fun getNonce(): BigInteger {
        currentNonce = if (currentNonce.signum() == -1) {
            // obtain lock
            super.getNonce()
        } else {
            currentNonce.add(BigInteger.ONE)
        }
        return currentNonce
    }

    @Synchronized
    @Throws(IOException::class)
    fun resetNonce() {
        currentNonce = super.getNonce()
    }

    @Synchronized
    fun setNonce(value: BigInteger) {
        currentNonce = value
    }
}
