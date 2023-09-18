// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

interface IChannelHandler {
    error Unregistered();
    error AlreadyRegistered();
    error InvalidCaller();
    error InvalidSignature();
    error InvalidPeersCount();
    error SignaturesNotEnough();
    error InvalidNonce();
    error InvalidMessagesLength();
    error SigParamsLengthMismatch(uint, uint, uint);
    error InsufficientGas();
    error LastPeer();

    struct Message {
        address target;
        uint256 max_gas;
        bytes payload;
    }
    struct Batch {
        uint256 nonce;
        // Must be equal to sum of `max_gas` in `messages`
        uint256 total_max_gas;
        Message[] messages;
    }

    /* Events */
    event MessageDispatched(address source, uint256 nonce, bytes payload);

    // Batch of messages was dispatched by relayer
    // - result - message results bitmap
    // - results_length - number of messages were dispatched
    // - gas_spent - gas spent for batch submission. Since event emitted before tx committed, actual gas is greater
    // (at least 10500 gas should be added).
    // - base fee - current block base fee.
    event BatchDispatched(
        uint256 batch_nonce,
        address relayer,
        uint256 results,
        uint256 results_length,
        uint256 gas_spent,
        uint256 base_fee
    );

    event ChangePeers(address peerId, bool removal);

    function submitMessage(bytes calldata payload) external;

    function submit(
        Batch calldata batch,
        uint8[] calldata v,
        bytes32[] calldata r,
        bytes32[] calldata s
    ) external;

    function removePeerByPeer(address peerAddress) external returns (bool);

    function addPeerByPeer(address peerAddress) external returns (bool);
}
