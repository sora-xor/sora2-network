package jp.co.soramitsu.soranet.eth.contract;

import io.reactivex.Flowable;
import io.reactivex.functions.Function;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.web3j.abi.EventEncoder;
import org.web3j.abi.FunctionEncoder;
import org.web3j.abi.TypeReference;
import org.web3j.abi.datatypes.Address;
import org.web3j.abi.datatypes.Bool;
import org.web3j.abi.datatypes.Event;
import org.web3j.abi.datatypes.Type;
import org.web3j.abi.datatypes.generated.Bytes32;
import org.web3j.crypto.Credentials;
import org.web3j.protocol.Web3j;
import org.web3j.protocol.core.DefaultBlockParameter;
import org.web3j.protocol.core.RemoteCall;
import org.web3j.protocol.core.RemoteFunctionCall;
import org.web3j.protocol.core.methods.request.EthFilter;
import org.web3j.protocol.core.methods.response.BaseEventResponse;
import org.web3j.protocol.core.methods.response.Log;
import org.web3j.protocol.core.methods.response.TransactionReceipt;
import org.web3j.tx.Contract;
import org.web3j.tx.TransactionManager;
import org.web3j.tx.gas.ContractGasProvider;

/**
 * <p>Auto generated code.
 * <p><strong>Do not modify!</strong>
 * <p>Please use the <a href="https://docs.web3j.io/command_line.html">web3j command line tools</a>,
 * or the org.web3j.codegen.SolidityFunctionWrapperGenerator in the 
 * <a href="https://github.com/web3j/web3j/tree/master/codegen">codegen module</a> to update.
 *
 * <p>Generated with web3j version 1.4.1.
 */
@SuppressWarnings("rawtypes")
public class NftMigration extends Contract {
    public static final String BINARY = "6080604052600180546001600160a01b031916733482549fca7511267c9ef7089507c0f16ea1dcc117905534801561003657600080fd5b5060405161041a38038061041a8339818101604052602081101561005957600080fd5b810190808051604051939291908464010000000082111561007957600080fd5b90830190602082018581111561008e57600080fd5b82518660208202830111640100000000821117156100ab57600080fd5b82525081516020918201928201910280838360005b838110156100d85781810151838201526020016100c0565b50505050919091016040525050600080546001600160a01b031916331781559150505b81518110156101505760016002600084848151811061011657fe5b6020908102919091018101516001600160a01b03168252810191909152604001600020805460ff19169115159190911790556001016100fb565b50506102b9806101616000396000f3fe608060405234801561001057600080fd5b506004361061004c5760003560e01c80635c170fd21461005157806361ef3ba0146100755780638da5cb5b146100af578063d9caa3d2146100b7575b600080fd5b6100596100d6565b604080516001600160a01b039092168252519081900360200190f35b61009b6004803603602081101561008b57600080fd5b50356001600160a01b03166100e5565b604080519115158252519081900360200190f35b6100596100fa565b6100d4600480360360208110156100cd57600080fd5b5035610109565b005b6001546001600160a01b031681565b60026020526000908152604090205460ff1681565b6000546001600160a01b031681565b6000546001600160a01b03163314156101535760405162461bcd60e51b81526004018080602001828103825260238152602001806102616023913960400191505060405180910390fd5b6001546001600160a01b031633141561019d5760405162461bcd60e51b81526004018080602001828103825260298152602001806102386029913960400191505060405180910390fd5b3360009081526002602052604090205460ff16610201576040805162461bcd60e51b815260206004820152601c60248201527f53656e6465722073686f756c642062652077686974656c697374656400000000604482015290519081900360640190fd5b6040805182815290517f4eb3aea69bf61684354f60a43d355c3026751ddd0ea4e1f5afc1274b96c655059181900360200190a15056fe53656e6465722073686f756c64206e6f74206265204e465420636f6e74726163742063726561746f7253656e6465722073686f756c64206e6f7420626520636f6e7472616374206f776e6572a26469706673582212207efff3cfed0c4945e9e63ef2d020a7130c1298f34ba5dd73d9aedf0b62890eac64736f6c63430007040033";

    public static final String FUNC_ACCEPTABLEADDRESSES = "acceptableAddresses";

    public static final String FUNC_NFTCREATOR = "nftCreator";

    public static final String FUNC_OWNER = "owner";

    public static final String FUNC_SUBMIT = "submit";

    public static final Event SUBMIT_EVENT = new Event("Submit", 
            Arrays.<TypeReference<?>>asList(new TypeReference<Bytes32>() {}));
    ;

    @Deprecated
    protected NftMigration(String contractAddress, Web3j web3j, Credentials credentials, BigInteger gasPrice, BigInteger gasLimit) {
        super(BINARY, contractAddress, web3j, credentials, gasPrice, gasLimit);
    }

    protected NftMigration(String contractAddress, Web3j web3j, Credentials credentials, ContractGasProvider contractGasProvider) {
        super(BINARY, contractAddress, web3j, credentials, contractGasProvider);
    }

    @Deprecated
    protected NftMigration(String contractAddress, Web3j web3j, TransactionManager transactionManager, BigInteger gasPrice, BigInteger gasLimit) {
        super(BINARY, contractAddress, web3j, transactionManager, gasPrice, gasLimit);
    }

    protected NftMigration(String contractAddress, Web3j web3j, TransactionManager transactionManager, ContractGasProvider contractGasProvider) {
        super(BINARY, contractAddress, web3j, transactionManager, contractGasProvider);
    }

    public List<SubmitEventResponse> getSubmitEvents(TransactionReceipt transactionReceipt) {
        List<Contract.EventValuesWithLog> valueList = extractEventParametersWithLog(SUBMIT_EVENT, transactionReceipt);
        ArrayList<SubmitEventResponse> responses = new ArrayList<SubmitEventResponse>(valueList.size());
        for (Contract.EventValuesWithLog eventValues : valueList) {
            SubmitEventResponse typedResponse = new SubmitEventResponse();
            typedResponse.log = eventValues.getLog();
            typedResponse.substrateAddress = (byte[]) eventValues.getNonIndexedValues().get(0).getValue();
            responses.add(typedResponse);
        }
        return responses;
    }

    public Flowable<SubmitEventResponse> submitEventFlowable(EthFilter filter) {
        return web3j.ethLogFlowable(filter).map(new Function<Log, SubmitEventResponse>() {
            @Override
            public SubmitEventResponse apply(Log log) {
                Contract.EventValuesWithLog eventValues = extractEventParametersWithLog(SUBMIT_EVENT, log);
                SubmitEventResponse typedResponse = new SubmitEventResponse();
                typedResponse.log = log;
                typedResponse.substrateAddress = (byte[]) eventValues.getNonIndexedValues().get(0).getValue();
                return typedResponse;
            }
        });
    }

    public Flowable<SubmitEventResponse> submitEventFlowable(DefaultBlockParameter startBlock, DefaultBlockParameter endBlock) {
        EthFilter filter = new EthFilter(startBlock, endBlock, getContractAddress());
        filter.addSingleTopic(EventEncoder.encode(SUBMIT_EVENT));
        return submitEventFlowable(filter);
    }

    public RemoteFunctionCall<Boolean> acceptableAddresses(String param0) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC_ACCEPTABLEADDRESSES, 
                Arrays.<Type>asList(new org.web3j.abi.datatypes.Address(160, param0)), 
                Arrays.<TypeReference<?>>asList(new TypeReference<Bool>() {}));
        return executeRemoteCallSingleValueReturn(function, Boolean.class);
    }

    public RemoteFunctionCall<String> nftCreator() {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC_NFTCREATOR, 
                Arrays.<Type>asList(), 
                Arrays.<TypeReference<?>>asList(new TypeReference<Address>() {}));
        return executeRemoteCallSingleValueReturn(function, String.class);
    }

    public RemoteFunctionCall<String> owner() {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(FUNC_OWNER, 
                Arrays.<Type>asList(), 
                Arrays.<TypeReference<?>>asList(new TypeReference<Address>() {}));
        return executeRemoteCallSingleValueReturn(function, String.class);
    }

    public RemoteFunctionCall<TransactionReceipt> submit(byte[] substrateAddress) {
        final org.web3j.abi.datatypes.Function function = new org.web3j.abi.datatypes.Function(
                FUNC_SUBMIT, 
                Arrays.<Type>asList(new org.web3j.abi.datatypes.generated.Bytes32(substrateAddress)), 
                Collections.<TypeReference<?>>emptyList());
        return executeRemoteCallTransaction(function);
    }

    @Deprecated
    public static NftMigration load(String contractAddress, Web3j web3j, Credentials credentials, BigInteger gasPrice, BigInteger gasLimit) {
        return new NftMigration(contractAddress, web3j, credentials, gasPrice, gasLimit);
    }

    @Deprecated
    public static NftMigration load(String contractAddress, Web3j web3j, TransactionManager transactionManager, BigInteger gasPrice, BigInteger gasLimit) {
        return new NftMigration(contractAddress, web3j, transactionManager, gasPrice, gasLimit);
    }

    public static NftMigration load(String contractAddress, Web3j web3j, Credentials credentials, ContractGasProvider contractGasProvider) {
        return new NftMigration(contractAddress, web3j, credentials, contractGasProvider);
    }

    public static NftMigration load(String contractAddress, Web3j web3j, TransactionManager transactionManager, ContractGasProvider contractGasProvider) {
        return new NftMigration(contractAddress, web3j, transactionManager, contractGasProvider);
    }

    public static RemoteCall<NftMigration> deploy(Web3j web3j, Credentials credentials, ContractGasProvider contractGasProvider, List<String> addresses) {
        String encodedConstructor = FunctionEncoder.encodeConstructor(Arrays.<Type>asList(new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.Address>(
                        org.web3j.abi.datatypes.Address.class,
                        org.web3j.abi.Utils.typeMap(addresses, org.web3j.abi.datatypes.Address.class))));
        return deployRemoteCall(NftMigration.class, web3j, credentials, contractGasProvider, BINARY, encodedConstructor);
    }

    public static RemoteCall<NftMigration> deploy(Web3j web3j, TransactionManager transactionManager, ContractGasProvider contractGasProvider, List<String> addresses) {
        String encodedConstructor = FunctionEncoder.encodeConstructor(Arrays.<Type>asList(new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.Address>(
                        org.web3j.abi.datatypes.Address.class,
                        org.web3j.abi.Utils.typeMap(addresses, org.web3j.abi.datatypes.Address.class))));
        return deployRemoteCall(NftMigration.class, web3j, transactionManager, contractGasProvider, BINARY, encodedConstructor);
    }

    @Deprecated
    public static RemoteCall<NftMigration> deploy(Web3j web3j, Credentials credentials, BigInteger gasPrice, BigInteger gasLimit, List<String> addresses) {
        String encodedConstructor = FunctionEncoder.encodeConstructor(Arrays.<Type>asList(new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.Address>(
                        org.web3j.abi.datatypes.Address.class,
                        org.web3j.abi.Utils.typeMap(addresses, org.web3j.abi.datatypes.Address.class))));
        return deployRemoteCall(NftMigration.class, web3j, credentials, gasPrice, gasLimit, BINARY, encodedConstructor);
    }

    @Deprecated
    public static RemoteCall<NftMigration> deploy(Web3j web3j, TransactionManager transactionManager, BigInteger gasPrice, BigInteger gasLimit, List<String> addresses) {
        String encodedConstructor = FunctionEncoder.encodeConstructor(Arrays.<Type>asList(new org.web3j.abi.datatypes.DynamicArray<org.web3j.abi.datatypes.Address>(
                        org.web3j.abi.datatypes.Address.class,
                        org.web3j.abi.Utils.typeMap(addresses, org.web3j.abi.datatypes.Address.class))));
        return deployRemoteCall(NftMigration.class, web3j, transactionManager, gasPrice, gasLimit, BINARY, encodedConstructor);
    }

    public static class SubmitEventResponse extends BaseEventResponse {
        public byte[] substrateAddress;
    }
}
