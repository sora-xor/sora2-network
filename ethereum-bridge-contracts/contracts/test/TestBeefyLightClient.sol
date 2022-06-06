// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "../BeefyLightClient.sol";

contract TestBeefyLightClient is BeefyLightClient {
    address public deployer;

    constructor(
        ValidatorRegistry _validatorRegistry,
        SimplifiedMMRVerification _mmrVerification,
        uint64 _startingBeefyBlock
    )
        BeefyLightClient(
            _validatorRegistry,
            _mmrVerification,
            _startingBeefyBlock
        )
    {
        deployer = msg.sender;
    }

    function reset(
        uint64 _startingBeefyBlock,
        bytes32 _authoritySetRoot,
        uint256 _authoritySetLen,
        uint64 _authoritySetId
    ) public {
        require(msg.sender == deployer, "Only deployer can reset contract");
        latestBeefyBlock = _startingBeefyBlock;
        latestMMRRoot = bytes32(0);
        validatorRegistry.update(
            _authoritySetRoot,
            _authoritySetLen,
            _authoritySetId
        );
    }
}
