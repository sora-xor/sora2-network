package net.sora.substrate;


import net.sora.substrate.bridge.contracts.Bridge;
import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.web3j.crypto.Credentials;
import org.web3j.protocol.Web3j;
import org.web3j.protocol.core.methods.response.TransactionReceipt;
import org.web3j.protocol.http.HttpService;
import org.web3j.tx.RawTransactionManager;
import org.web3j.tx.TransactionManager;
import org.web3j.tx.Transfer;
import org.web3j.tx.gas.DefaultGasProvider;
import org.web3j.utils.Convert;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;

public class BaseNetworkTest {

    private final Logger log = LoggerFactory.getLogger(this.getClass());

    private static final String RECIPIENT = "0x67c359dFC4c0eFE6F841Da202B48Bb80b6e6DCAf";
    private static final BigInteger GAS_PRICE = new BigInteger("20000000000");
    private static final BigInteger GAS_LIMIT = new BigInteger("6721975");

    private static Credentials getCredentialsFromPrivateKey() {
        return Credentials.create("bd2aaab3f68fd7fd81d5167231ce6aeb23df29642e8cf62772af4d96afdef894");
    }

    /*
    Have to setup user accounts correctly before run the test
     */
    //@Test
    public void sendTransfer() throws Exception {
        Web3j web3j = Web3j.build(new HttpService());

        TransactionManager tm = new RawTransactionManager(web3j, getCredentialsFromPrivateKey());

        Transfer transfer = new Transfer(web3j, tm);
        TransactionReceipt tr = transfer.sendFunds(
                RECIPIENT,
                BigDecimal.ONE,
                Convert.Unit.ETHER,
                GAS_PRICE,
                GAS_LIMIT).send();
    }

   // @Test
    public void printWeb3j() throws Exception {
        Web3j web3j = Web3j.build(new HttpService());

        log.info("Connected to Ethereum client version: "
                + web3j.web3ClientVersion().send().getWeb3ClientVersion());
    }
}
