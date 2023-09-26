// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "./libraries/Bits.sol";
import "./libraries/Bitfield.sol";
import "./libraries/ScaleCodec.sol";
import "./interfaces/ISimplifiedMMRProof.sol";
import "./interfaces/ISimplifiedMMRVerification.sol";
import "./libraries/MerkleProof.sol";

/**
 * @title A entry contract for the Ethereum light client
 */
contract BeefyLightClient is ISimplifiedMMRProof, Ownable {
    using Bits for uint256;
    using Bitfield for uint256[];
    using ScaleCodec for uint256;
    using ScaleCodec for uint64;
    using ScaleCodec for uint32;
    using ScaleCodec for uint16;

    /* Events */
    /**
     * @notice Notifies an observer that the complete verification process has
     *  finished successfully and the new commitmentHash will be accepted
     * @param prover The address of the successful prover
     * @param blockNumber commitment block number
     */
    event VerificationSuccessful(address prover, uint32 blockNumber);

    event NewMMRRoot(bytes32 mmrRoot, uint64 blockNumber);

    /* Types */

    /**
     * The Commitment, with its payload, is the core thing we are trying to verify with
     * this contract. It contains a MMR root that commits to the Polkadot history, including
     * past blocks and parachain blocks and can be used to verify both Polkadot and parachain blocks.
     * @param payload the payload of the new commitment in beefy justifications (in
     * our case, this is a new MMR root for all past Polkadot blocks)
     * @param blockNumber block number for the given commitment
     * @param validatorSetId validator set id that signed the given commitment
     */
    struct Commitment {
        bytes payloadPrefix;
        bytes32 payload;
        bytes payloadSuffix;
        uint32 blockNumber;
        uint64 validatorSetId;
    }

    /**
     * The ValidatorProof is a collection of proofs used to verify the signatures from the validators signing
     * each new justification.
     * @param signatures an array of signatures from the randomly chosen validators
     * @param positions an array of the positions of the randomly chosen validators
     * @param publicKeys an array of the public key of each signer
     * @param publicKeyMerkleProofs an array of merkle proofs from the chosen validators proving that their public
     * keys are in the validator set
     */
    struct ValidatorProof {
        uint256[] validatorClaimsBitfield;
        bytes[] signatures;
        uint256[] positions;
        address[] publicKeys;
        bytes32[][] publicKeyMerkleProofs;
    }

    /**
     * The BeefyMMRLeaf is the structure of each leaf in each MMR that each commitment's payload commits to.
     * @param version version of the leaf type
     * @param parentNumber parent number of the block this leaf describes
     * @param parentHash parent hash of the block this leaf describes
     * @param nextAuthoritySetId validator set id that will be part of consensus for the next block
     * @param nextAuthoritySetLen length of that validator set
     * @param nextAuthoritySetRoot merkle root of all public keys in that validator set
     * @param randomHash BABE VRF randomness for the block this leaf describes
     * @param digestHash hash of the latest finalized block
     */
    struct BeefyMMRLeaf {
        uint8 version;
        uint32 parentNumber;
        uint64 nextAuthoritySetId;
        uint32 nextAuthoritySetLen; // More tightly packed, `version` 1byte, `parentNumber` 4byte,
        // `nextAuthoritySetId` 8byte,
        // `nextAuthoritySetLen` 4byte now use single storage slot.
        bytes32 parentHash;
        bytes32 nextAuthoritySetRoot;
        bytes32 randomSeed;
        bytes32 digestHash;
    }

    /**
     * @dev The ValidatorSet describes a BEEFY validator set
     * @param id identifier for the set
     * @param root Merkle root of BEEFY validator addresses
     * @param length number of validators in the set
     */
    struct ValidatorSet {
        uint128 id;
        uint128 length;
        bytes32 root;
    }

    /* State */
    ISimplifiedMMRVerification public mmrVerification;

    // Ring buffer of latest MMR Roots
    mapping(uint256 => bytes32) public latestMMRRoots;
    uint32 public latestMMRRootIndex; // default value is 0
    uint32 public constant MMR_ROOT_HISTORY_SIZE = 30;

    uint64 public latestBeefyBlock;
    bytes32 public latestRandomSeed;

    ValidatorSet public currentValidatorSet;
    ValidatorSet public nextValidatorSet;

    /* Constants */

    // THRESHOLD_NUMERATOR - numerator for percent of validator signatures required
    // THRESHOLD_DENOMINATOR - denominator for percent of validator signatures required
    uint256 public constant THRESHOLD_NUMERATOR = 22;
    uint256 public constant THRESHOLD_DENOMINATOR = 59;

    // We must ensure at least one block is processed every session,
    // so these constants are checked to enforce a maximum gap between commitments.
    uint64 public constant NUMBER_OF_BLOCKS_PER_SESSION = 600;
    uint64 public constant ERROR_AND_SAFETY_BUFFER = 10;
    uint64 public constant MAXIMUM_BLOCK_GAP =
        NUMBER_OF_BLOCKS_PER_SESSION + ERROR_AND_SAFETY_BUFFER;

    bytes2 public constant MMR_ROOT_ID = 0x6d68;

    /**
     * @notice Deploys the BeefyLightClient contract
     * @param _mmrVerification The contract to be used for MMR verification
     */
    constructor(address _mmrVerification) {
        mmrVerification = ISimplifiedMMRVerification(_mmrVerification);
        latestRandomSeed = bytes32(uint256(42));
    }

    /* Public Functions */
    function initialize(
        uint64 startingBeefyBlock,
        ValidatorSet calldata _currentValidatorSet,
        ValidatorSet calldata _nextValidatorSet
    ) external onlyOwner {
        currentValidatorSet = _currentValidatorSet;
        nextValidatorSet = _nextValidatorSet;
        latestBeefyBlock = startingBeefyBlock;
        renounceOwnership();
    }

    /**
     * @notice Adds MMR root to the known last roots history.
     */
    function addKnownMMRRoot(bytes32 root) public returns (uint32 index) {
        uint32 newRootIndex = (latestMMRRootIndex + 1) % MMR_ROOT_HISTORY_SIZE;
        latestMMRRoots[newRootIndex] = root;
        latestMMRRootIndex = newRootIndex;
        return latestMMRRootIndex;
    }

    /**
     * @notice Whether the root is present in the root history
     */
    function isKnownRoot(bytes32 root) public view returns (bool) {
        if (root == 0) {
            return false;
        }
        uint32 i = latestMMRRootIndex;
        do {
            if (root == latestMMRRoots[i]) {
                return true;
            }
            if (i == 0) {
                i = MMR_ROOT_HISTORY_SIZE;
            }
            i--;
        } while (i != latestMMRRootIndex);
        return false;
    }

    /**
     *@notice Returns the last added root
     */
    function getLatestMMRRoot() external view returns (bytes32) {
        return latestMMRRoots[latestMMRRootIndex];
    }

    /**
     * @notice Executed by the incoming channel in order to verify commitment
     * @param beefyMMRLeaf contains the merkle leaf to be verified
     * @param proof contains simplified MMR proof
     */
    function verifyBeefyMerkleLeaf(
        bytes32 beefyMMRLeaf,
        SimplifiedMMRProof memory proof
    ) external view returns (bool) {
        bytes32 proofRoot = mmrVerification.calculateMerkleRoot(
            beefyMMRLeaf,
            proof.merkleProofItems,
            proof.merkleProofOrderBitField
        );

        return isKnownRoot(proofRoot);
    }

    function createRandomBitfield(
        uint256[] memory validatorClaimsBitfield,
        uint256 numberOfValidators
    ) external view returns (uint256[] memory) {
        return
            Bitfield.randomNBitsWithPriorCheck(
                getSeed(),
                validatorClaimsBitfield,
                requiredNumberOfSignatures(numberOfValidators),
                numberOfValidators
            );
    }

    function createInitialBitfield(
        uint256[] calldata bitsToSet,
        uint256 length
    ) external pure returns (uint256[] memory) {
        return Bitfield.createBitfield(bitsToSet, length);
    }

    /**
     * @notice Submit a new BEEFY commitment to the light client
     * @param commitment contains the full commitment that was used for the commitmentHash
     * @param validatorProof a struct containing the data needed to verify all validator signatures
     * @param latestMMRLeaf the merkle leaf that was used to create the latestMMRRoot
     * @param proof contains the simplified MMR proof for the latestMMRLeaf
     */
    function submitSignatureCommitment(
        Commitment calldata commitment,
        ValidatorProof calldata validatorProof,
        BeefyMMRLeaf calldata latestMMRLeaf,
        SimplifiedMMRProof calldata proof
    ) external {
        ValidatorSet memory vset;
        if (commitment.validatorSetId == currentValidatorSet.id) {
            vset = currentValidatorSet;
        } else if (commitment.validatorSetId == nextValidatorSet.id) {
            vset = nextValidatorSet;
        } else {
            revert("Invalid validator set id");
        }
        verifyCommitment(vset, commitment, validatorProof);
        verifyNewestMMRLeaf(latestMMRLeaf, commitment.payload, proof);

        processPayload(commitment.payload, commitment.blockNumber);

        latestRandomSeed = latestMMRLeaf.randomSeed;

        emit VerificationSuccessful(msg.sender, commitment.blockNumber);
        applyValidatorSetChanges(
            latestMMRLeaf.nextAuthoritySetId,
            latestMMRLeaf.nextAuthoritySetLen,
            latestMMRLeaf.nextAuthoritySetRoot
        );
    }

    /* Private Functions */

    /**
     * @return onChainRandNums an array storing the random numbers generated inside this function
     */
    function getSeed() private view returns (uint256) {
        // @note Create hash of block number and random seed
        bytes32 randomSeedWithBlockNumber = keccak256(
            bytes.concat(latestRandomSeed, bytes8(latestBeefyBlock))
        );

        return uint256(randomSeedWithBlockNumber);
    }

    function verifyNewestMMRLeaf(
        BeefyMMRLeaf calldata leaf,
        bytes32 root,
        SimplifiedMMRProof calldata proof
    ) public view {
        bytes memory encodedLeaf = encodeMMRLeaf(leaf);
        bytes32 hashedLeaf = hashMMRLeaf(encodedLeaf);

        require(
            mmrVerification.verifyInclusionProof(root, hashedLeaf, proof),
            "invalid mmr proof"
        );
    }

    /**
     * @notice Perform some operation[s] using the payload
     * @param payload The payload variable passed in via the initial function
     */
    function processPayload(bytes32 payload, uint64 blockNumber) private {
        // Check that payload.leaf.block_number is > last_known_block_number;
        require(
            blockNumber > latestBeefyBlock,
            "Payload blocknumber is too old"
        );

        // Check that payload is within the current or next session
        // to ensure we get at least one payload each session
        require(
            blockNumber < latestBeefyBlock + MAXIMUM_BLOCK_GAP,
            "Payload blocknumber is too new"
        );

        addKnownMMRRoot(payload);
        latestBeefyBlock = blockNumber;
        emit NewMMRRoot(payload, blockNumber);
    }

    /**
     * @notice Check if the payload includes a new validator set,
     * and if it does then update the new validator set
     * @dev This function should call out to the validator registry contract
     * @param nextAuthoritySetId The id of the next authority set
     * @param nextAuthoritySetLen The number of validators in the next authority set
     * @param nextAuthoritySetRoot The merkle root of the merkle tree of the next validators
     */
    function applyValidatorSetChanges(
        uint128 nextAuthoritySetId,
        uint128 nextAuthoritySetLen,
        bytes32 nextAuthoritySetRoot
    ) internal {
        if (nextAuthoritySetId != nextValidatorSet.id) {
            require(
                nextAuthoritySetId > nextValidatorSet.id,
                "Error: Cannot switch to old validator set"
            );
            currentValidatorSet = nextValidatorSet;
            nextValidatorSet = ValidatorSet({
                id: nextAuthoritySetId,
                length: nextAuthoritySetLen,
                root: nextAuthoritySetRoot
            });
        }
    }

    function requiredNumberOfSignatures() external view returns (uint256) {
        return requiredNumberOfSignatures(currentValidatorSet.length);
    }

    function requiredNumberOfSignatures(
        uint256 numValidators
    ) public pure returns (uint256) {
        return
            (numValidators * THRESHOLD_NUMERATOR + THRESHOLD_DENOMINATOR - 1) /
            THRESHOLD_DENOMINATOR;
    }

    /**
     * @dev https://github.com/sora-xor/substrate/blob/7d914ce3ed34a27d7bb213caed374d64cde8cfa8/client/beefy/src/round.rs#L62
     */
    function checkCommitmentSignaturesThreshold(
        uint256 numOfValidators,
        uint256[] calldata validatorClaimsBitfield
    ) public pure {
        uint256 threshold = numOfValidators - (numOfValidators - 1) / 3;
        require(
            Bitfield.countSetBits(validatorClaimsBitfield) >= threshold,
            "Error: Not enough validator signatures"
        );
    }

    function verifyCommitment(
        ValidatorSet memory vset,
        Commitment calldata commitment,
        ValidatorProof calldata proof
    ) internal view {
        uint256 numberOfValidators = vset.length;
        uint256 requiredNumOfSignatures = requiredNumberOfSignatures(
            numberOfValidators
        );

        checkCommitmentSignaturesThreshold(
            numberOfValidators,
            proof.validatorClaimsBitfield
        );

        uint256[] memory randomBitfield = Bitfield.randomNBitsWithPriorCheck(
            getSeed(),
            proof.validatorClaimsBitfield,
            requiredNumOfSignatures,
            numberOfValidators
        );

        verifyValidatorProofLengths(requiredNumOfSignatures, proof);

        // Encode and hash the commitment
        bytes32 commitmentHash = createCommitmentHash(commitment);

        verifyValidatorProofSignatures(
            randomBitfield,
            proof,
            requiredNumOfSignatures,
            commitmentHash
        );
    }

    function verifyValidatorProofLengths(
        uint256 requiredNumOfSignatures,
        ValidatorProof calldata proof
    ) internal pure {
        /**
         * @dev verify that required number of signatures, positions, public keys and merkle proofs are
         * submitted
         */
        require(
            proof.signatures.length == requiredNumOfSignatures,
            "Error: Number of signatures does not match required"
        );
        require(
            proof.positions.length == requiredNumOfSignatures,
            "Error: Number of validator positions does not match required"
        );
        require(
            proof.publicKeys.length == requiredNumOfSignatures,
            "Error: Number of validator public keys does not match required"
        );
        require(
            proof.publicKeyMerkleProofs.length == requiredNumOfSignatures,
            "Error: Number of validator public keys does not match required"
        );
    }

    function verifyValidatorProofSignatures(
        uint256[] memory randomBitfield,
        ValidatorProof calldata proof,
        uint256 requiredNumOfSignatures,
        bytes32 commitmentHash
    ) internal view {
        /**
         *  @dev For each randomSignature, do:
         */
        for (uint256 i = 0; i < requiredNumOfSignatures; i++) {
            verifyValidatorSignature(
                randomBitfield,
                proof.signatures[i],
                proof.positions[i],
                proof.publicKeys[i],
                proof.publicKeyMerkleProofs[i],
                commitmentHash
            );
        }
    }

    function verifyValidatorSignature(
        uint256[] memory randomBitfield,
        bytes calldata signature,
        uint256 position,
        address publicKey,
        bytes32[] calldata publicKeyMerkleProof,
        bytes32 commitmentHash
    ) internal view {
        /**
         * @dev Check if validator in randomBitfield
         */
        require(
            randomBitfield.isSet(position),
            "Error: Validator must be once in bitfield"
        );

        /**
         * @dev Remove validator from randomBitfield such that no validator can appear twice in signatures
         */
        randomBitfield.clear(position);

        /**
         * @dev Check if merkle proof is valid
         */
        require(
            checkValidatorInSet(publicKey, position, publicKeyMerkleProof),
            "Error: Validator must be in validator set at correct position"
        );

        /**
         * @dev Check if signature is correct
         */
        require(
            ECDSA.recover(commitmentHash, signature) == publicKey,
            "Error: Invalid Signature"
        );
    }

    function createCommitmentHash(
        Commitment calldata commitment
    ) public pure returns (bytes32) {
        return
            keccak256(
                bytes.concat(
                    commitment.payloadPrefix,
                    MMR_ROOT_ID,
                    bytes1(0x80), // Vec len: 32
                    commitment.payload,
                    commitment.payloadSuffix,
                    commitment.blockNumber.encode32(),
                    commitment.validatorSetId.encode64()
                )
            );
    }

    function encodeMMRLeaf(
        BeefyMMRLeaf calldata leaf
    ) public pure returns (bytes memory) {
        bytes memory scaleEncodedMMRLeaf = abi.encodePacked(
            ScaleCodec.encode8(leaf.version),
            ScaleCodec.encode32(leaf.parentNumber),
            leaf.parentHash,
            ScaleCodec.encode64(leaf.nextAuthoritySetId),
            ScaleCodec.encode32(leaf.nextAuthoritySetLen),
            leaf.nextAuthoritySetRoot,
            leaf.randomSeed,
            leaf.digestHash
        );

        return scaleEncodedMMRLeaf;
    }

    function hashMMRLeaf(bytes memory leaf) public pure returns (bytes32) {
        return keccak256(leaf);
    }

    /**
     * @notice Checks if a validators address is a member of the merkle tree
     * @param addr The address of the validator to check
     * @param pos The position of the validator to check, index starting at 0
     * @param proof Merkle proof required for validation of the address
     * @return Returns true if the validator is in the set
     */
    function checkValidatorInSet(
        address addr,
        uint256 pos,
        bytes32[] memory proof
    ) public view returns (bool) {
        bytes32 hashedLeaf = keccak256(abi.encodePacked(addr));
        ValidatorSet memory vset = currentValidatorSet;
        return
            MerkleProof.verifyMerkleLeafAtPosition(
                vset.root,
                hashedLeaf,
                pos,
                vset.length,
                proof
            );
    }
}
