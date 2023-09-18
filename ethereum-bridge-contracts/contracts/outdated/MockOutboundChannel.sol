// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;
pragma experimental ABIEncoderV2;

import "./IOutboundChannel.sol";

contract MockOutboundChannel is IOutboundChannel {
    function submit(address, bytes calldata) external override {}
}
