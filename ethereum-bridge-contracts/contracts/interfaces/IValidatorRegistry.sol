// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

interface IValidatorRegistry {
    /* Events */
    event ValidatorRegistryUpdated(
        bytes32 root,
        uint256 numOfValidators,
        uint64 id
    );

    function update(
        bytes32 _root,
        uint256 _numOfValidators,
        uint64 _id
    ) external;

    function checkValidatorInSet(
        address addr,
        uint256 pos,
        bytes32[] memory proof
    ) external view returns (bool);

    function numOfValidators() external view returns (uint);
    function root() external view returns (bytes32);
    function id() external view returns (uint64);
}
