// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.7.6;
pragma experimental ABIEncoderV2;

import "../IOutboundChannel.sol";

contract MockOutboundChannel is IOutboundChannel {
    function submit(address, bytes calldata) external override {}

    function fee() external pure override returns (uint256) {
        return 0;
    }
}
