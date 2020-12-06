package net.sora.substrate.bridge.contracts;

import io.reactivex.Flowable;
import io.reactivex.functions.Function;
import org.web3j.abi.EventEncoder;
import org.web3j.abi.TypeReference;
import org.web3j.abi.datatypes.Address;
import org.web3j.abi.datatypes.Bool;
import org.web3j.abi.datatypes.Event;
import org.web3j.abi.datatypes.Type;
import org.web3j.abi.datatypes.generated.Bytes32;
import org.web3j.abi.datatypes.generated.Uint256;
import org.web3j.crypto.Credentials;
import org.web3j.protocol.Web3j;
import org.web3j.protocol.core.DefaultBlockParameter;
import org.web3j.protocol.core.RemoteFunctionCall;
import org.web3j.protocol.core.methods.request.EthFilter;
import org.web3j.protocol.core.methods.response.BaseEventResponse;
import org.web3j.protocol.core.methods.response.Log;
import org.web3j.protocol.core.methods.response.TransactionReceipt;
import org.web3j.tx.Contract;
import org.web3j.tx.TransactionManager;
import org.web3j.tx.gas.ContractGasProvider;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

/**
 * <p>Auto generated code.
 * <p><strong>Do not modify!</strong>
 * <p>Please use the <a href="https://docs.web3j.io/command_line.html">web3j command line tools</a>,
 * or the org.web3j.codegen.SolidityFunctionWrapperGenerator in the 
 * <a href="https://github.com/web3j/web3j/tree/master/codegen">codegen module</a> to update.
 *
 * <p>Generated with web3j version 4.5.16.
 */
@SuppressWarnings("rawtypes")
public class Bridge extends Contract {
    public static final String BINARY = "Bin file was not provided";

    public static final String FUNC__SIDECHAINTOKENS = "_sidechainTokens";

    public static final String FUNC__SIDECHAINTOKENSBYADDRESS = "_sidechainTokensByAddress";

    public static final String FUNC__UNIQUEADDRESSES = "_uniqueAddresses";

    public static final String FUNC_ADDNEWSIDECHAINTOKEN = "addNewSidechainToken";

    public static final String FUNC_ADDPEERBYPEER = "addPeerByPeer";

    public static final String FUNC_ISPEER = "isPeer";

    public static final String FUNC_PEERSCOUNT = "peersCount";

    public static final String FUNC_RECEIVEBYETHEREUMASSETADDRESS = "receiveByEthereumAssetAddress";

    public static final String FUNC_RECEIVEBYSIDECHAINASSETID = "receiveBySidechainAssetId";

    public static final String FUNC_REMOVEPEERBYPEER = "removePeerByPeer";

    public static final String FUNC_SENDERC20TOSIDECHAIN = "sendERC20ToSidechain";

    public static final String FUNC_SENDETHTOSIDECHAIN = "sendEthToSidechain";

    public static final String FUNC_SHUTDOWN = "shutDown";

    public static final String FUNC_USED = "used";

    public static final Event DEPOSIT_EVENT = new Event("Deposit", 
            Arrays.<TypeReference<?>>asList(new TypeReference<Bytes32>() {}, new TypeReference<Uint256>() {}, new TypeReference<Address>() {}, new TypeReference<Bytes32>() {}));
    ;

    public static final Event WITHDRAWAL_EVENT = new Event("Withdrawal", 
            Arrays.<TypeReference<?>>asList(new TypeReference<Bytes32>() {}));
    ;

    @Deprecated
    protected Bridge(String contractAddress, Web3j web3j, Credentials credentials, BigInteger gasPrice, BigInteger gasLimit) {
        super(BINARY, contractAddress, web3j, credentials, gasPrice, gasLimit);
    }

    protected Bridge(String contractAddress, Web3j web3j, Credentials credentials, ContractGasProvider contractGasProvider) {
        super(BINARY, contractAddress, web3j, credentials, contractGasProvider);
    }

    @Deprecated
    protected Bridge(String contractAddress, Web3j web3j, TransactionManager transactionManager, BigInteger gasPrice, BigInteger gasLimit) {
        super(BINARY, contractAddress, web3j, transactionManager, gasPrice, gasLimit);
    }

    protected Bridge(String contractAddress, Web3j web3j, TransactionManager transactionManager, ContractGasProvider contractGasProvider) {
        super(BINARY, contractAddress, web3j, transactionManager, contractGasProvider);
    }

    public List<DepositEventResponse> getDepositEvents(TransactionReceipt transactionReceipt) {
        List<EventValuesWithLog> valueList = extractEventParametersWithLog(DEPOSIT_EVENT, transactionReceipt);
        ArrayList<DepositEventResponse> responses = new ArrayList<DepositEventResponse>(valueList.size());
        for (EventValuesWithLog eventValues : valueList) {
            DepositEventResponse typedResponse = new DepositEventResponse();
            typedResponse.log = eventValues.getLog();
            typedResponse.destination = (byte[]) eventValues.getNonIndexedValues().get(0).getValue();
            typedResponse.amount = (BigInteger) eventValues.getNonIndexedValues().get(1).getValue();
            typedResponse.token = (String) eventValues.getNonIndexedValues().get(2).getValue();
            typedResponse.sidechainAsset = (byte[]) eventValues.getNonIndexedValues().get(3).getValue();
            responses.add(typedResponse);
        }
        return responses;
    }

    public Flowable<DepositEventResponse> depositEventFlowable(EthFilter filter) {
        return web3j.ethLogFlowable(filter).map(new Function<Log, DepositEventResponse>() {
            @Override
            public DepositEventResponse apply(Log log) {
                EventValuesWithLog eventValues = extractEventParametersWithLog(DEPOSIT_EVENT, log);
                DepositEventResponse typedResponse = new DepositEventResponse();
                typedResponse.log = log;
                typedResponse.destination = (byte[]) eventValues.getNonIndexedValues().get(0).getValue();
                typedResponse.amount = (BigInteger) eventValues.getNonIndexedValues().get(1).getValue();
                typedResponse.token = (String) eventValues.getNonIndexedValues().get(2).getValue();
                typedResponse.sidechainAsset = (byte[]) eventValues.getNonIndexedValues().get(3).getValue();
                return typedResponse;
            }
        });
    }

    public Flowable<DepositEventResponse> depositEventFlowable(DefaultBlockParameter startBlock, DefaultBlockParameter endBlock) {
        EthFilter filter = new EthFilter(startBlock, endBlock, getContractAddress());
        filter.addSingleTopic(EventEncoder.encode(DEPOSIT_EVENT));
        return depositEventFlowable(filter);
    }

    public List<WithdrawalEventResponse> getWithdrawalEvents(TransactionReceipt transactionReceipt) {
        List<EventValuesWithLog> valueList = extractEventParametersWithLog(WITHDRAWAL_EVENT, transactionReceipt);
        ArrayList<WithdrawalEventResponse> responses = new ArrayList<WithdrawalEventResponse>(valueList.size());
        for (EventValuesWithLog eventValues : valueList) {
            WithdrawalEventResponse typedResponse = new WithdrawalEventResponse();
            typedResponse.log = eventValues.getLog();
            typedResponse.txHash = (byte[]) eventValues.getNonIndexedValues().get(0).getValue();
            responses.add(typedResponse);
        }
        return responses;
    }

    public Flowable<WithdrawalEventResponse> withdrawalEventFlowable(EthFilter filter) {
        return web3j.ethLogFlowable(filter).map(new Function<Log, WithdrawalEventResponse>() {
            @Override
            public WithdrawalEventResponse apply(Log log) {
                EventValuesWithLog eventValues = extractEventParametersWithLog(WITHDRAWAL_EVENT, log);
                WithdrawalEventResponse typedResponse = new WithdrawalEventResponse();
                typedResponse.log = log;
                typedResponse.txHash = (byte[]) eventValues.getNonIndexedValues().get(0).getValue();
                return typedResponse;
            }
        });
    }

    public Flowable<WithdrawalEventResponse> withdrawalEventFlowable(DefaultBlockParameter startBlock, DefaultBlockParameter endBlock) {
        EthFilter filter = new EthFilter(startBlock, endBlock, getContractAddress());
        filter.addSingleTopic(EventEncoder.encode(WITHDRAWAL_EVENT));
        return withdrawalEventFlowable(filter);
    }

    public RemoteFunctionCall<String> _sidechainTokens(byte[] param0) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC__SIDECHAINTOKENS, 
                Arrays.<Type>asList(new Bytes32(param0)),
                Arrays.<TypeReference<?>>asList(new TypeReference<Address>() {}));
        return executeRemoteCallSingleValueReturn(function, String.class);
    }

    public RemoteFunctionCall<byte[]> _sidechainTokensByAddress(String param0) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC__SIDECHAINTOKENSBYADDRESS, 
                Arrays.<Type>asList(new Address(160, param0)),
                Arrays.<TypeReference<?>>asList(new TypeReference<Bytes32>() {}));
        return executeRemoteCallSingleValueReturn(function, byte[].class);
    }

    public RemoteFunctionCall<Boolean> _uniqueAddresses(String param0) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC__UNIQUEADDRESSES, 
                Arrays.<Type>asList(new Address(160, param0)),
                Arrays.<TypeReference<?>>asList(new TypeReference<Bool>() {}));
        return executeRemoteCallSingleValueReturn(function, Boolean.class);
    }

    public RemoteFunctionCall<TransactionReceipt> addNewSidechainToken(String name, String symbol, BigInteger decimals, BigInteger supply, byte[] sidechainAssetId, List<BigInteger> v, List<byte[]> r, List<byte[]> s) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_ADDNEWSIDECHAINTOKEN, 
                Arrays.<Type>asList(new org.web3j.abi.datatypes.Utf8String(name), 
                new org.web3j.abi.datatypes.Utf8String(symbol), 
                new org.web3j.abi.datatypes.generated.Uint8(decimals), 
                new Uint256(supply),
                new Bytes32(sidechainAssetId),
                new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.generated.Uint8>(
                        org.web3j.abi.datatypes.generated.Uint8.class,
                        org.web3j.abi.Utils.typeMap(v, org.web3j.abi.datatypes.generated.Uint8.class)), 
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(r, Bytes32.class)),
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(s, Bytes32.class))),
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    public RemoteFunctionCall<TransactionReceipt> addPeerByPeer(String newPeerAddress, byte[] txHash, List<BigInteger> v, List<byte[]> r, List<byte[]> s) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_ADDPEERBYPEER, 
                Arrays.<Type>asList(new Address(160, newPeerAddress),
                new Bytes32(txHash),
                new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.generated.Uint8>(
                        org.web3j.abi.datatypes.generated.Uint8.class,
                        org.web3j.abi.Utils.typeMap(v, org.web3j.abi.datatypes.generated.Uint8.class)), 
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(r, Bytes32.class)),
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(s, Bytes32.class))),
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    public RemoteFunctionCall<Boolean> isPeer(String param0) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC_ISPEER, 
                Arrays.<Type>asList(new Address(160, param0)),
                Arrays.<TypeReference<?>>asList(new TypeReference<Bool>() {}));
        return executeRemoteCallSingleValueReturn(function, Boolean.class);
    }

    public RemoteFunctionCall<BigInteger> peersCount() {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC_PEERSCOUNT, 
                Arrays.<Type>asList(), 
                Arrays.<TypeReference<?>>asList(new TypeReference<Uint256>() {}));
        return executeRemoteCallSingleValueReturn(function, BigInteger.class);
    }

    public RemoteFunctionCall<TransactionReceipt> receiveByEthereumAssetAddress(String tokenAddress, BigInteger amount, String to, byte[] txHash, List<BigInteger> v, List<byte[]> r, List<byte[]> s, String from) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_RECEIVEBYETHEREUMASSETADDRESS, 
                Arrays.<Type>asList(new Address(160, tokenAddress),
                new Uint256(amount),
                new Address(160, to),
                new Bytes32(txHash),
                new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.generated.Uint8>(
                        org.web3j.abi.datatypes.generated.Uint8.class,
                        org.web3j.abi.Utils.typeMap(v, org.web3j.abi.datatypes.generated.Uint8.class)), 
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(r, Bytes32.class)),
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(s, Bytes32.class)),
                new Address(160, from)),
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    public RemoteFunctionCall<TransactionReceipt> receiveBySidechainAssetId(byte[] sidechainAssetId, BigInteger amount, String beneficiary, byte[] txHash, List<BigInteger> v, List<byte[]> r, List<byte[]> s, String from) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_RECEIVEBYSIDECHAINASSETID, 
                Arrays.<Type>asList(new Bytes32(sidechainAssetId),
                new Uint256(amount),
                new Address(160, beneficiary),
                new Bytes32(txHash),
                new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.generated.Uint8>(
                        org.web3j.abi.datatypes.generated.Uint8.class,
                        org.web3j.abi.Utils.typeMap(v, org.web3j.abi.datatypes.generated.Uint8.class)), 
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(r, Bytes32.class)),
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(s, Bytes32.class)),
                new Address(160, from)),
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    public RemoteFunctionCall<TransactionReceipt> removePeerByPeer(String peerAddress, byte[] txHash, List<BigInteger> v, List<byte[]> r, List<byte[]> s) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_REMOVEPEERBYPEER, 
                Arrays.<Type>asList(new Address(160, peerAddress),
                new Bytes32(txHash),
                new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.generated.Uint8>(
                        org.web3j.abi.datatypes.generated.Uint8.class,
                        org.web3j.abi.Utils.typeMap(v, org.web3j.abi.datatypes.generated.Uint8.class)), 
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(r, Bytes32.class)),
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(s, Bytes32.class))),
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    public RemoteFunctionCall<TransactionReceipt> sendERC20ToSidechain(byte[] destination, BigInteger amount, String tokenAddress) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_SENDERC20TOSIDECHAIN, 
                Arrays.<Type>asList(new Bytes32(destination),
                new Uint256(amount),
                new Address(160, tokenAddress)),
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    public RemoteFunctionCall<TransactionReceipt> sendEthToSidechain(byte[] destination) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_SENDETHTOSIDECHAIN, 
                Arrays.<Type>asList(new Bytes32(destination)),
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    public RemoteFunctionCall<TransactionReceipt> shutDown(String thisContractAddress, String salt, List<BigInteger> v, List<byte[]> r, List<byte[]> s) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_SHUTDOWN, 
                Arrays.<Type>asList(new Address(160, thisContractAddress),
                new org.web3j.abi.datatypes.Utf8String(salt), 
                new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.generated.Uint8>(
                        org.web3j.abi.datatypes.generated.Uint8.class,
                        org.web3j.abi.Utils.typeMap(v, org.web3j.abi.datatypes.generated.Uint8.class)), 
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(r, Bytes32.class)),
                new org.web3j.abi.datatypes.DynamicArray<Bytes32>(
                        Bytes32.class,
                        org.web3j.abi.Utils.typeMap(s, Bytes32.class))),
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    public RemoteFunctionCall<Boolean> used(byte[] param0) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC_USED, 
                Arrays.<Type>asList(new Bytes32(param0)),
                Arrays.<TypeReference<?>>asList(new TypeReference<Bool>() {}));
        return executeRemoteCallSingleValueReturn(function, Boolean.class);
    }

    @Deprecated
    public static Bridge load(String contractAddress, Web3j web3j, Credentials credentials, BigInteger gasPrice, BigInteger gasLimit) {
        return new Bridge(contractAddress, web3j, credentials, gasPrice, gasLimit);
    }

    @Deprecated
    public static Bridge load(String contractAddress, Web3j web3j, TransactionManager transactionManager, BigInteger gasPrice, BigInteger gasLimit) {
        return new Bridge(contractAddress, web3j, transactionManager, gasPrice, gasLimit);
    }

    public static Bridge load(String contractAddress, Web3j web3j, Credentials credentials, ContractGasProvider contractGasProvider) {
        return new Bridge(contractAddress, web3j, credentials, contractGasProvider);
    }

    public static Bridge load(String contractAddress, Web3j web3j, TransactionManager transactionManager, ContractGasProvider contractGasProvider) {
        return new Bridge(contractAddress, web3j, transactionManager, contractGasProvider);
    }

    public static class DepositEventResponse extends BaseEventResponse {
        public byte[] destination;

        public BigInteger amount;

        public String token;

        public byte[] sidechainAsset;
    }

    public static class WithdrawalEventResponse extends BaseEventResponse {
        public byte[] txHash;
    }
}
