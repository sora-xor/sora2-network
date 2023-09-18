// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

interface IBatch {
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
}
