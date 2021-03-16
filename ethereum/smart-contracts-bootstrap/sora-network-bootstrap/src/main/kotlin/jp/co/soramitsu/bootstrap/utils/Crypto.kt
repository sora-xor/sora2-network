/*
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

package jp.co.soramitsu.bootstrap.sidechain.util

import org.apache.commons.codec.binary.Hex
import org.web3j.crypto.ECKeyPair
import org.web3j.crypto.Hash
import org.web3j.crypto.Sign
import org.web3j.utils.Numeric
import java.math.BigInteger

/**
 * Prepare data to sign for Ethereum contract
 */
fun prepareDataToSign(toSign: String): ByteArray {
    // Message from hex to bytes
    val dat = Numeric.hexStringToByteArray(toSign)
    // Add ethereum signature format
    return ("\u0019Ethereum Signed Message:\n" + (dat.size)).toByteArray() + dat
}

/**
 * Signs user-provided data with predefined account deployed on local Parity node
 * @param ecKeyPair keypair used to sign
 * @param toSign data to sign
 * @return signed data
 */
fun signUserData(ecKeyPair: ECKeyPair, toSign: String): String {
    val dataToSign = prepareDataToSign(toSign)
    val signature = Sign.signMessage(dataToSign, ecKeyPair)
    // Combine in the signature
    var res = Numeric.toHexString(signature.r)
    res = res.plus(Numeric.toHexString(signature.s).substring(2))
    //  The v is always either 27, or 28 - need to convert to 00 or 01
    res = res.plus("0" + (BigInteger(signature.v).toInt() - 27).toString())
    return res
}



/**
 * Calculates keccak-256 hash of several params concatenation. Params are:
 * @param peerAddress Ethereum address of notary
 * @param irohaHash hash of transaction in Iroha
 * @return keccak-256 hash of all provided fields
 */
fun hashToAddAndRemovePeer(
    peerAddress: String,
    irohaHash: String
): String {
    return Hash.sha3(
        peerAddress.replace("0x", "")
                + irohaHash.replace("0x", "")
    )
}

/**
 * Data class which stores signature splitted into components
 * @param v v component of signature
 * @param r r component of signature
 * @param s s component of signature
 */
data class VRS(val v: BigInteger, val r: ByteArray, val s: ByteArray)

/**
 * Data class which stores signature splitted into components
 * @param v v component of signature
 * @param r r component of signature in hex
 * @param s s component of signature in hex
 */
data class VRSSignature(val v: String, val r: String, val s: String)

/**
 * Extracts VRS-signature from string-encoded signature
 * @param signature string-encoded signature
 * @return VRS object
 */
fun extractVRS(signature: String): VRS {
    val r = Numeric.hexStringToByteArray(signature.substring(2, 66))
    val s = Numeric.hexStringToByteArray(signature.substring(66, 130))
    var v = signature.substring(130, 132).toBigInteger(16)
    if (v == BigInteger.valueOf(0) || v == BigInteger.valueOf(1)) {
        v += BigInteger.valueOf(27)
    }
    return VRS(v, r, s)
}

/**
 * Convert to Ethereum-compatible format of VRS Signature
 */
fun extractVRSSignature(signature: String): VRSSignature {
    val vrs = extractVRS(signature)
    val v = vrs.v.toString(16).replace("0x", "")
    val r = Hex.encodeHexString(vrs.r).replace("0x", "")
    val s = Hex.encodeHexString(vrs.s).replace("0x", "")
    return VRSSignature(v, r, s)
}
