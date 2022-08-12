// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;
pragma experimental ABIEncoderV2;

import "../interfaces/IOutboundChannel.sol";

contract MockOutboundChannel is IOutboundChannel {
    function submit(address, bytes calldata) external override {}

    function fee() external pure override returns (uint256) {
        return 0;
    }
}
