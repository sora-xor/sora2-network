// SPDX-License-Identifier: BSD-4-Clause
pragma solidity ^0.8.23;

import {ISccpVerifier} from "../ISccpVerifier.sol";

/// @notice Verifier that always succeeds. Intended for local testing only.
contract AlwaysTrueVerifier is ISccpVerifier {
    function verifyBurnProof(
        uint32,
        bytes32,
        bytes calldata,
        bytes calldata
    ) external pure returns (bool) {
        return true;
    }

    function verifyTokenAddProof(bytes32, bytes calldata, bytes calldata) external pure returns (bool) {
        return true;
    }

    function verifyTokenPauseProof(bytes32, bytes calldata, bytes calldata) external pure returns (bool) {
        return true;
    }

    function verifyTokenResumeProof(bytes32, bytes calldata, bytes calldata) external pure returns (bool) {
        return true;
    }
}
