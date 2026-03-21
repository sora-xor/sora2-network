// SPDX-License-Identifier: BSD-4-Clause
//
// Minimal contract used for validating SCCP codec + keccak256 in TVM.

import "messages"
import "sccp-codec"
import "errors"

type Allowed = TopUpTons

const MAX_U32 = 0xffffffff
const MAX_U256 = 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff

fun onInternalMessage(in: InMessage) {
    val _ = lazy Allowed.fromSlice(in.body);
    // no-op
}

get fun get_message_id(
    sourceDomain: int,
    destDomain: int,
    nonce: int,
    soraAssetId: int,
    amount: int,
    recipient32: int,
): uint256 {
    assert (sourceDomain >= 0 && sourceDomain <= MAX_U32) throw ERROR_SCCP_DOMAIN_UNSUPPORTED;
    assert (destDomain >= 0 && destDomain <= MAX_U32) throw ERROR_SCCP_DOMAIN_UNSUPPORTED;
    assert (nonce >= 0 && nonce <= MAX_U64) throw ERROR_SCCP_NONCE_OVERFLOW;
    assert (soraAssetId >= 0 && soraAssetId <= MAX_U256) throw ERROR_INVALID_MESSAGE;
    assert (recipient32 >= 0 && recipient32 <= MAX_U256) throw ERROR_INVALID_MESSAGE;
    assert (amount >= 0 && amount <= MAX_U128) throw ERROR_SCCP_AMOUNT_TOO_LARGE;
    return sccpBurnMessageIdV1(
        sourceDomain as uint32,
        destDomain as uint32,
        nonce as uint64,
        soraAssetId as uint256,
        amount as coins,
        recipient32 as uint256,
    );
}
