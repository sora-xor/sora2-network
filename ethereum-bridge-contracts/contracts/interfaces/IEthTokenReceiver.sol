// SPDX-License-Identifier: Apache License 2.0

pragma solidity 0.8.15;

interface IEthTokenReceiver {
    function receivePayment() external payable;
}
