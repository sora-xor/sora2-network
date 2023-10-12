// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "./BeefyLightClient.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestBeefyLightClient is BeefyLightClient {
    constructor(address testMMRVerification)
        BeefyLightClient(testMMRVerification)
    {}

    function reset(
        uint64 startingBeefyBlock,
        ValidatorSet calldata currentValidatorSet_,
        ValidatorSet calldata nextValidatorSet_
    ) external onlyOwner {
        currentValidatorSet = currentValidatorSet_;
        nextValidatorSet = nextValidatorSet_;
        latestBeefyBlock = startingBeefyBlock;
        latestMMRRoots[0] = bytes32(0);
        latestMMRRootIndex = 0;
    }
}
