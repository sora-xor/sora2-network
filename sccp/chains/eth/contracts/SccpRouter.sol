// SPDX-License-Identifier: BSD-4-Clause
pragma solidity ^0.8.23;

import {ISccpVerifier} from "./ISccpVerifier.sol";
import {SccpCodec} from "./SccpCodec.sol";
import {SccpToken} from "./SccpToken.sol";

/// @notice SCCP router for EVM chains.
///
/// Burns and mints are permissionless, while token lifecycle controls (add/pause/resume)
/// are driven only by SORA-finalized BEEFY proofs.
contract SccpRouter {
    using SccpCodec for bytes;

    // Domain ids (must match SORA pallet constants).
    uint32 public constant DOMAIN_SORA = 0;
    uint32 public constant DOMAIN_ETH = 1;
    uint32 public constant DOMAIN_BSC = 2;
    uint32 public constant DOMAIN_SOL = 3;
    uint32 public constant DOMAIN_TON = 4;
    uint32 public constant DOMAIN_TRON = 5;
    bytes32 public constant BURN_EVENT_TOPIC0 =
        keccak256("SccpBurned(bytes32,bytes32,address,uint128,uint32,bytes32,uint64,bytes)");

    error ZeroAddress();
    error TokenAlreadyRegistered();
    error TokenNotRegistered();
    error TokenNotActive();
    error TokenNotPaused();
    error AmountIsZero();
    error RecipientIsZero();
    error DomainUnsupported();
    error DomainEqualsLocal();
    error NonceOverflow();
    error BurnRecordAlreadyExists();
    error BurnRecordNotFound();
    error InboundAlreadyProcessed();
    error GovernanceActionAlreadyProcessed();
    error ProofVerificationFailed();
    error AmountTooLarge();
    error RecipientNotCanonical();
    error InvalidGovernancePayload();
    error TokenMetadataInvalid();

    enum TokenState {
        None,
        Active,
        Paused
    }

    event VerifierConfigured(address indexed verifier);

    event TokenAddedByProof(bytes32 indexed messageId, bytes32 indexed soraAssetId, address token, uint8 decimals);
    event TokenPausedByProof(bytes32 indexed messageId, bytes32 indexed soraAssetId);
    event TokenResumedByProof(bytes32 indexed messageId, bytes32 indexed soraAssetId);

    /// @notice Canonical outbound burn proof target for SCCP 20-byte-address domains -> SORA flows.
    /// @dev Off-chain proof builders must treat the emitted payload bytes and indexed message id
    ///      as protocol-critical and stable for `messageId = keccak256("sccp:burn:v1" || payload)`.
    event SccpBurned(
        bytes32 indexed messageId,
        bytes32 indexed soraAssetId,
        address indexed sender,
        uint128 amount,
        uint32 destDomain,
        bytes32 recipient,
        uint64 nonce,
        bytes payload
    );

    event SccpMinted(bytes32 indexed messageId, bytes32 indexed soraAssetId, address indexed recipient, uint128 amount);

    struct BurnRecord {
        address sender;
        bytes32 soraAssetId;
        uint128 amount;
        uint32 destDomain;
        bytes32 recipient;
        uint64 nonce;
        uint64 blockNumber;
    }

    // forge-lint: disable-next-line(screaming-snake-case-immutable)
    uint32 public immutable localDomain;
    // forge-lint: disable-next-line(screaming-snake-case-immutable)
    ISccpVerifier public immutable verifier;

    uint64 public outboundNonce;

    mapping(bytes32 => address) public tokenBySoraAssetId;
    mapping(bytes32 => TokenState) public tokenStateBySoraAssetId;
    mapping(bytes32 => BurnRecord) public burns;

    mapping(bytes32 => bool) public processedInbound;
    mapping(bytes32 => bool) public processedGovernanceMessage;

    constructor(uint32 localDomain_, address verifier_) {
        if (verifier_ == address(0)) revert ZeroAddress();
        _ensureSupportedDomain(localDomain_);
        localDomain = localDomain_;
        verifier = ISccpVerifier(verifier_);
        emit VerifierConfigured(verifier_);
    }

    /// @notice Burn wrapped tokens on this chain to create a burn message for `destDomain`.
    /// @dev Caller must `approve(router, amount)` on the wrapped token first.
    function burnToDomain(
        bytes32 soraAssetId,
        uint256 amount,
        uint32 destDomain,
        bytes32 recipient
    ) external returns (bytes32 messageId) {
        if (amount == 0) revert AmountIsZero();
        if (recipient == bytes32(0)) revert RecipientIsZero();
        if (destDomain == localDomain) revert DomainEqualsLocal();
        _ensureSupportedDomain(destDomain);

        // If the destination uses a 20-byte address format, enforce canonical encoding:
        // address right-aligned in 32 bytes and non-zero.
        if (_isEvmDomain(destDomain)) {
            if ((uint256(recipient) >> 160) != 0) revert RecipientNotCanonical();
            if (address(uint160(uint256(recipient))) == address(0)) revert RecipientIsZero();
        }

        address token = tokenBySoraAssetId[soraAssetId];
        if (token == address(0)) revert TokenNotRegistered();
        if (tokenStateBySoraAssetId[soraAssetId] != TokenState.Active) revert TokenNotActive();

        if (outboundNonce == type(uint64).max) revert NonceOverflow();
        outboundNonce += 1;

        if (amount > type(uint128).max) revert AmountTooLarge();
        // forge-lint: disable-next-line(unsafe-typecast)
        uint128 amt = uint128(amount);

        SccpCodec.BurnPayloadV1 memory p = SccpCodec.BurnPayloadV1({
            version: 1,
            sourceDomain: localDomain,
            destDomain: destDomain,
            nonce: outboundNonce,
            soraAssetId: soraAssetId,
            amount: amt,
            recipient: recipient
        });
        bytes memory payload = SccpCodec.encodeBurnPayloadV1(p);
        messageId = SccpCodec.burnMessageId(payload);

        if (burns[messageId].sender != address(0)) revert BurnRecordAlreadyExists();

        SccpToken(token).burnFrom(msg.sender, amount);

        burns[messageId] = BurnRecord({
            sender: msg.sender,
            soraAssetId: soraAssetId,
            amount: amt,
            destDomain: destDomain,
            recipient: recipient,
            nonce: outboundNonce,
            blockNumber: uint64(block.number)
        });

        emit SccpBurned(messageId, soraAssetId, msg.sender, amt, destDomain, recipient, outboundNonce, payload);
    }

    /// @notice Reconstruct the canonical payload bytes for a burn record.
    function burnPayload(bytes32 messageId) external view returns (bytes memory payload) {
        BurnRecord memory r = burns[messageId];
        if (r.sender == address(0)) revert BurnRecordNotFound();
        SccpCodec.BurnPayloadV1 memory p = SccpCodec.BurnPayloadV1({
            version: 1,
            sourceDomain: localDomain,
            destDomain: r.destDomain,
            nonce: r.nonce,
            soraAssetId: r.soraAssetId,
            amount: r.amount,
            recipient: r.recipient
        });
        payload = SccpCodec.encodeBurnPayloadV1(p);
    }

    /// @notice Mint wrapped tokens on this chain based on a verified burn on `sourceDomain`.
    function mintFromProof(uint32 sourceDomain, bytes calldata payload, bytes calldata proof) external {
        _ensureSupportedDomain(sourceDomain);
        if (sourceDomain == localDomain) revert DomainEqualsLocal();

        bytes32 messageId = SccpCodec.burnMessageId(payload);
        if (processedInbound[messageId]) revert InboundAlreadyProcessed();

        SccpCodec.BurnPayloadV1 memory p = SccpCodec.decodeBurnPayloadV1(payload);
        if (p.version != 1) revert DomainUnsupported();
        if (p.sourceDomain != sourceDomain) revert DomainUnsupported();
        if (p.destDomain != localDomain) revert DomainUnsupported();
        if (p.amount == 0) revert AmountIsZero();
        if (p.recipient == bytes32(0)) revert RecipientIsZero();

        address token = tokenBySoraAssetId[p.soraAssetId];
        if (token == address(0)) revert TokenNotRegistered();
        if (tokenStateBySoraAssetId[p.soraAssetId] != TokenState.Active) revert TokenNotActive();

        bool ok = verifier.verifyBurnProof(sourceDomain, messageId, payload, proof);
        if (!ok) revert ProofVerificationFailed();

        // 20-byte address recipient encoding: right-aligned in a 32-byte field.
        if ((uint256(p.recipient) >> 160) != 0) revert RecipientNotCanonical();
        address recipient = address(uint160(uint256(p.recipient)));
        if (recipient == address(0)) revert RecipientIsZero();
        SccpToken(token).mint(recipient, uint256(p.amount));

        processedInbound[messageId] = true;
        emit SccpMinted(messageId, p.soraAssetId, recipient, p.amount);
    }

    /// @notice Add and activate a token mapping based on a SORA-finalized governance proof.
    function addTokenFromProof(bytes calldata payload, bytes calldata proof) external returns (address token) {
        bytes32 messageId = SccpCodec.tokenAddMessageId(payload);
        if (processedGovernanceMessage[messageId]) revert GovernanceActionAlreadyProcessed();

        SccpCodec.TokenAddPayloadV1 memory p = SccpCodec.decodeTokenAddPayloadV1(payload);
        if (p.version != 1) revert InvalidGovernancePayload();
        if (p.targetDomain != localDomain) revert DomainUnsupported();
        if (tokenBySoraAssetId[p.soraAssetId] != address(0)) revert TokenAlreadyRegistered();

        bool ok = verifier.verifyTokenAddProof(messageId, payload, proof);
        if (!ok) revert ProofVerificationFailed();

        string memory name = _bytes32ToString(p.name);
        string memory symbol = _bytes32ToString(p.symbol);
        if (bytes(name).length == 0 || bytes(symbol).length == 0) revert TokenMetadataInvalid();

        SccpToken t = new SccpToken(name, symbol, p.decimals, address(this));
        token = address(t);
        tokenBySoraAssetId[p.soraAssetId] = token;
        tokenStateBySoraAssetId[p.soraAssetId] = TokenState.Active;
        processedGovernanceMessage[messageId] = true;

        emit TokenAddedByProof(messageId, p.soraAssetId, token, p.decimals);
    }

    /// @notice Pause a registered token based on a SORA-finalized governance proof.
    function pauseTokenFromProof(bytes calldata payload, bytes calldata proof) external {
        bytes32 messageId = SccpCodec.tokenPauseMessageId(payload);
        if (processedGovernanceMessage[messageId]) revert GovernanceActionAlreadyProcessed();

        SccpCodec.TokenControlPayloadV1 memory p = SccpCodec.decodeTokenPausePayloadV1(payload);
        if (p.version != 1) revert InvalidGovernancePayload();
        if (p.targetDomain != localDomain) revert DomainUnsupported();
        if (tokenBySoraAssetId[p.soraAssetId] == address(0)) revert TokenNotRegistered();
        if (tokenStateBySoraAssetId[p.soraAssetId] != TokenState.Active) revert TokenNotActive();

        bool ok = verifier.verifyTokenPauseProof(messageId, payload, proof);
        if (!ok) revert ProofVerificationFailed();

        tokenStateBySoraAssetId[p.soraAssetId] = TokenState.Paused;
        processedGovernanceMessage[messageId] = true;

        emit TokenPausedByProof(messageId, p.soraAssetId);
    }

    /// @notice Resume a paused token based on a SORA-finalized governance proof.
    function resumeTokenFromProof(bytes calldata payload, bytes calldata proof) external {
        bytes32 messageId = SccpCodec.tokenResumeMessageId(payload);
        if (processedGovernanceMessage[messageId]) revert GovernanceActionAlreadyProcessed();

        SccpCodec.TokenControlPayloadV1 memory p = SccpCodec.decodeTokenResumePayloadV1(payload);
        if (p.version != 1) revert InvalidGovernancePayload();
        if (p.targetDomain != localDomain) revert DomainUnsupported();
        if (tokenBySoraAssetId[p.soraAssetId] == address(0)) revert TokenNotRegistered();
        if (tokenStateBySoraAssetId[p.soraAssetId] != TokenState.Paused) revert TokenNotPaused();

        bool ok = verifier.verifyTokenResumeProof(messageId, payload, proof);
        if (!ok) revert ProofVerificationFailed();

        tokenStateBySoraAssetId[p.soraAssetId] = TokenState.Active;
        processedGovernanceMessage[messageId] = true;

        emit TokenResumedByProof(messageId, p.soraAssetId);
    }

    function _ensureSupportedDomain(uint32 domain) internal pure {
        if (
            domain != DOMAIN_SORA &&
            domain != DOMAIN_ETH &&
            domain != DOMAIN_BSC &&
            domain != DOMAIN_SOL &&
            domain != DOMAIN_TON &&
            domain != DOMAIN_TRON
        ) revert DomainUnsupported();
    }

    function _isEvmDomain(uint32 domain) internal pure returns (bool) {
        return domain == DOMAIN_ETH || domain == DOMAIN_BSC || domain == DOMAIN_TRON;
    }

    function _bytes32ToString(bytes32 raw) internal pure returns (string memory out) {
        bytes memory b = new bytes(32);
        uint256 len = 0;
        uint256 zeroAt = type(uint256).max;

        for (uint256 i = 0; i < 32; i++) {
            bytes1 c = raw[i];
            if (c == bytes1(0)) {
                zeroAt = i;
                break;
            }
            uint8 v = uint8(c);
            if (v < 0x20 || v > 0x7E) revert TokenMetadataInvalid();
            b[len] = c;
            len += 1;
        }

        if (len == 0) revert TokenMetadataInvalid();

        if (zeroAt != type(uint256).max) {
            for (uint256 j = zeroAt + 1; j < 32; j++) {
                if (raw[j] != bytes1(0)) revert TokenMetadataInvalid();
            }
        }

        out = new string(len);
        bytes memory outB = bytes(out);
        for (uint256 k = 0; k < len; k++) {
            outB[k] = b[k];
        }
    }
}
