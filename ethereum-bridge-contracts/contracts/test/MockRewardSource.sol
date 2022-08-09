// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;
pragma experimental ABIEncoderV2;

import "../interfaces/IRewardSource.sol";

contract MockRewardSource is IRewardSource {
    function reward(address payable, uint256 _amount) pure external override {
        // Simulate the case where there are no funds to reward the relayer
        require(_amount != 1024, "No funds available");
    }
}
