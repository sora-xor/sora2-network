// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "../BeefyLightClient.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestBeefyLightClient is BeefyLightClient, Ownable {
    constructor(
        address testValidatorRegistry,
        address tetsMMRVerification,
        uint64 startingBeefyBlock
    )
        BeefyLightClient(
            testValidatorRegistry,
            tetsMMRVerification,
            startingBeefyBlock
        )
    {}

    function reset(
        uint64 startingBeefyBlock,
        bytes32 authoritySetRoot,
        uint256 authoritySetLen,
        uint64 authoritySetId
    ) external onlyOwner {
        latestBeefyBlock = startingBeefyBlock;
        latestMMRRoots[0] = bytes32(0);
        latestMMRRootIndex = 0;
        validatorRegistry.update(
            authoritySetRoot,
            authoritySetLen,
            authoritySetId
        );
    }
}
