// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "../BeefyLightClient.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract TestBeefyLightClient is BeefyLightClient, Ownable {
    constructor(
        address _validatorRegistry,
        address _mmrVerification,
        uint64 _startingBeefyBlock
    )
        BeefyLightClient(
            _validatorRegistry,
            _mmrVerification,
            _startingBeefyBlock
        )
    {}

    function reset(
        uint64 _startingBeefyBlock,
        bytes32 _authoritySetRoot,
        uint256 _authoritySetLen,
        uint64 _authoritySetId
    ) external onlyOwner {
        latestBeefyBlock = _startingBeefyBlock;
        latestMMRRoots[0] = bytes32(0);
        latestMMRRootIndex = 0;
        validatorRegistry.update(
            _authoritySetRoot,
            _authoritySetLen,
            _authoritySetId
        );
    }
}
