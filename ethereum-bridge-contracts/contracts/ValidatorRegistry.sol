// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/access/Ownable.sol";
import "./libraries/MerkleProof.sol";
import "./interfaces/IValidatorRegistry.sol";

/**
 * @title A contract storing state on the current validator set
 * @dev Stores the validator set as a Merkle root
 * @dev Inherits `Ownable` to ensure it can only be callable by the
 * instantiating contract account (which is the BeefyLightClient contract)
 */
contract ValidatorRegistry is IValidatorRegistry, Ownable {
    /* State */

    bytes32 public root;
    uint256 public numOfValidators;
    uint64 public id;

    /**
     * @notice Updates the validator registry and number of validators
     * @param newRoot The new root
     * @param newNumOfValidators The new number of validators
     */
    function update(
        bytes32 newRoot,
        uint256 newNumOfValidators,
        uint64 newId
    ) external override onlyOwner {
        root = newRoot;
        numOfValidators = newNumOfValidators;
        id = newId;
        emit ValidatorRegistryUpdated(root, numOfValidators, id);
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
    ) external view override returns (bool) {
        bytes32 hashedLeaf = keccak256(abi.encodePacked(addr));
        return
            MerkleProof.verifyMerkleLeafAtPosition(
                root,
                hashedLeaf,
                pos,
                numOfValidators,
                proof
            );
    }
}
