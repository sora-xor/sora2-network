// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

interface IOutboundChannel {
    /* Events */
    event Message(address source, uint64 nonce, bytes payload);

    function submit(address origin, bytes calldata payload) external;
}
