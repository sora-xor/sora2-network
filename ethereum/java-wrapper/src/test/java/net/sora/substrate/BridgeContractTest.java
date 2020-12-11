package net.sora.substrate;

import net.sora.substrate.bridge.contracts.Bridge;
import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.web3j.crypto.Credentials;
import org.web3j.protocol.Web3j;
import org.web3j.protocol.http.HttpService;
import org.web3j.tx.RawTransactionManager;
import org.web3j.tx.TransactionManager;
import org.web3j.tx.gas.DefaultGasProvider;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;

import static org.junit.Assert.*;

public class BridgeContractTest {

    private final Logger log = LoggerFactory.getLogger(this.getClass());

    private static Credentials getCredentialsFromPrivateKey() {
        return Credentials.create("bd2aaab3f68fd7fd81d5167231ce6aeb23df29642e8cf62772af4d96afdef894");
    }

    @Test
    public void bridgeTest() throws Exception {
        Web3j web3j = Web3j.build(new HttpService());
        Credentials creds = getCredentialsFromPrivateKey();
        TransactionManager tm = new RawTransactionManager(web3j, creds);

        List<String> initialPeers = new ArrayList<>();
        initialPeers.add(creds.getAddress());

        Bridge bridge = Bridge.deploy(web3j, tm, new DefaultGasProvider(), initialPeers).send();
        log.info("Contract Bridge address: " + bridge.getContractAddress());

        // Test is peer
        assertTrue(bridge.isPeer(creds.getAddress()).send());
        // Test is not a peer
        String notPeer = "0x13209947ECc4257A3e9A7ac5f11F513Ba0B82D21";
        assertFalse(bridge.isPeer(notPeer).send());
        // Test Peers count
        assertEquals(BigInteger.ONE, bridge.peersCount().send());
    }
}
