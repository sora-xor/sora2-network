## `ValidatorRegistry`



Stores the validator set as a Merkle root
Inherits `Ownable` to ensure it can only be callable by the
instantiating contract account (which is the BeefyLightClient contract)


### `update(bytes32 _root, uint256 _numOfValidators, uint64 _id)` (public)

Updates the validator registry and number of validators




### `checkValidatorInSet(address addr, uint256 pos, bytes32[] proof) → bool` (public)

Checks if a validators address is a member of the merkle tree





### `ValidatorRegistryUpdated(bytes32 root, uint256 numOfValidators, uint64 id)`





