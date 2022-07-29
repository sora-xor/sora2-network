## `BeefyLightClient`






### `constructor(contract ValidatorRegistry _validatorRegistry, contract SimplifiedMMRVerification _mmrVerification, uint64 _startingBeefyBlock)` (public)

Deploys the BeefyLightClient contract




### `verifyBeefyMerkleLeaf(bytes32 beefyMMRLeaf, struct SimplifiedMMRProof proof) → bool` (external)

Executed by the incoming channel in order to verify commitment




### `newSignatureCommitment(bytes32 commitmentHash, uint256[] validatorClaimsBitfield, bytes validatorSignature, uint256 validatorPosition, address validatorPublicKey, bytes32[] validatorPublicKeyMerkleProof)` (public)

Executed by the prover in order to begin the process of block
acceptance by the light client




### `createRandomBitfield(uint256 id) → uint256[]` (public)





### `createInitialBitfield(uint256[] bitsToSet, uint256 length) → uint256[]` (public)





### `completeSignatureCommitment(uint256 id, struct BeefyLightClient.Commitment commitment, struct BeefyLightClient.ValidatorProof validatorProof, struct BeefyLightClient.BeefyMMRLeaf latestMMRLeaf, struct SimplifiedMMRProof proof)` (public)

Performs the second step in the validation logic




### `verifyNewestMMRLeaf(struct BeefyLightClient.BeefyMMRLeaf leaf, bytes32 root, struct SimplifiedMMRProof proof)` (public)





### `applyValidatorSetChanges(uint64 nextAuthoritySetId, uint32 nextAuthoritySetLen, bytes32 nextAuthoritySetRoot)` (internal)

Check if the payload includes a new validator set,
and if it does then update the new validator set


This function should call out to the validator registry contract


### `requiredNumberOfSignatures() → uint256` (public)





### `requiredNumberOfSignatures(uint256 numValidators) → uint256` (public)





### `verifyCommitment(uint256 id, struct BeefyLightClient.Commitment commitment, struct BeefyLightClient.ValidatorProof proof)` (internal)





### `verifyValidatorProofLengths(uint256 requiredNumOfSignatures, struct BeefyLightClient.ValidatorProof proof)` (internal)





### `verifyValidatorProofSignatures(uint256[] randomBitfield, struct BeefyLightClient.ValidatorProof proof, uint256 requiredNumOfSignatures, struct BeefyLightClient.Commitment commitment)` (internal)





### `verifyValidatorSignature(uint256[] randomBitfield, bytes signature, uint256 position, address publicKey, bytes32[] publicKeyMerkleProof, bytes32 commitmentHash)` (internal)





### `createCommitmentHash(struct BeefyLightClient.Commitment commitment) → bytes32` (public)





### `encodeMMRLeaf(struct BeefyLightClient.BeefyMMRLeaf leaf) → bytes` (public)





### `hashMMRLeaf(bytes leaf) → bytes32` (public)






### `InitialVerificationSuccessful(address prover, uint256 blockNumber, uint256 id)`

Notifies an observer that the prover's attempt at initital
verification was successful.


Note that the prover must wait until `n` blocks have been mined
subsequent to the generation of this event before the 2nd tx can be sent


### `FinalVerificationSuccessful(address prover, uint256 id)`

Notifies an observer that the complete verification process has
 finished successfuly and the new commitmentHash will be accepted




### `NewMMRRoot(bytes32 mmrRoot, uint64 blockNumber)`





