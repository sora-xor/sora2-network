// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

error Unregistered();
error AlreadyRegistered();
error InvalidCaller();
error InvalidSignature();
error InvalidPeersCount();
error SignaturesNotEnough();
error InvalidNonce();
error InvalidLength();
error SigParamsLengthMismatch(uint, uint, uint);
error InsufficientGas();
error LastPeer();

error InvalidRecipient();
error InvalidAmount();
error FailedCall();

error InvalidContract();
