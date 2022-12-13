// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "../BeefyLightClient.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestBeefyLightClient is BeefyLightClient {
    constructor(address testMMRVerification)
        BeefyLightClient(testMMRVerification)
    {}

    function reset(
        uint64 startingBeefyBlock,
        ValidatorSet calldata _currentValidatorSet,
        ValidatorSet calldata _nextValidatorSet
    ) external onlyOwner {
        currentValidatorSet = _currentValidatorSet;
        nextValidatorSet = _nextValidatorSet;
        latestBeefyBlock = startingBeefyBlock;
        latestMMRRoots[0] = bytes32(0);
        latestMMRRootIndex = 0;
    }
}
