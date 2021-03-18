/*
 * Copyright D3 Ledger, Inc. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

package jp.co.soramitsu.bootstrap.utils

import jp.co.soramitsu.bootstrap.dto.SigsData
import jp.co.soramitsu.bootstrap.sidechain.util.extractVRS
import jp.co.soramitsu.bootstrap.sidechain.util.signUserData
import org.web3j.crypto.ECKeyPair
import org.web3j.crypto.Hash
import org.web3j.utils.Numeric
import java.math.BigInteger

val defaultIrohaHash = Hash.sha3(String.format("%064x", BigInteger.valueOf(12345)))
val defaultByteHash = irohaHashToByteHash(defaultIrohaHash)

fun irohaHashToByteHash(irohaHash: String) =
    Numeric.hexStringToByteArray(irohaHash.slice(2 until irohaHash.length))

fun prepareSignatures(amount: Int, keypairs: List<ECKeyPair>, toSign: String): SigsData {
    val vv = ArrayList<BigInteger>()
    val rr = ArrayList<ByteArray>()
    val ss = ArrayList<ByteArray>()

    for (i in 0 until amount) {
        val signature = signUserData(keypairs[i], toSign)
        val vrs = extractVRS(signature)
        vv.add(vrs.v)
        rr.add(vrs.r)
        ss.add(vrs.s)
    }
    return SigsData(vv, rr, ss)
}
