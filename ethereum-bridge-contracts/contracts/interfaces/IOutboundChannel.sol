// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

interface IOutboundChannel {
    /* Events */
    event Message(address source, uint64 nonce, uint256 fee, bytes payload);
    event FeeChanged(uint256 oldFee, uint256 newFee);

    function submit(address origin, bytes calldata payload) external;
    function fee() external view returns (uint256);
}
