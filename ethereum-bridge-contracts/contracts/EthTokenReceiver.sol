// SPDX-License-Identifier: Apache License 2.0

pragma solidity =0.8.13;

interface EthTokenReceiver {
    function receivePayment() external payable;
}
