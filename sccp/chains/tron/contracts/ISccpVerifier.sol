// SPDX-License-Identifier: BSD-4-Clause
pragma solidity ^0.8.23;

/// @notice On-chain verifier hook for SCCP burn proofs.
/// @dev Implementations are chain-specific (light client / consensus proofs / etc).
interface ISccpVerifier {
    /// @notice Verify that `payload` corresponds to a burn on `sourceDomain` with id `messageId`.
    /// @dev Must be deterministic and side-effect free. Should return `false` on verification failure.
    function verifyBurnProof(
        uint32 sourceDomain,
        bytes32 messageId,
        bytes calldata payload,
        bytes calldata proof
    ) external view returns (bool);

    /// @notice Verify a SORA-finalized governance proof that adds a token on this domain.
    function verifyTokenAddProof(bytes32 messageId, bytes calldata payload, bytes calldata proof)
        external
        view
        returns (bool);

    /// @notice Verify a SORA-finalized governance proof that pauses a token on this domain.
    function verifyTokenPauseProof(bytes32 messageId, bytes calldata payload, bytes calldata proof)
        external
        view
        returns (bool);

    /// @notice Verify a SORA-finalized governance proof that resumes a token on this domain.
    function verifyTokenResumeProof(bytes32 messageId, bytes calldata payload, bytes calldata proof)
        external
        view
        returns (bool);
}
