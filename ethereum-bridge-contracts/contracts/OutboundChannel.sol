// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

interface OutboundChannel {
    function submit(address origin, bytes calldata payload) external;

    function fee() external view returns (uint256);
}
