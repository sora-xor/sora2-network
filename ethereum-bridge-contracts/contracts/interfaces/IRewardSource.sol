// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

// Something that can reward a relayer
interface IRewardSource {
    event Rewarded(address recipient, uint256 amount);

    // Should not revert transaction on insufficient funds
    function reward(address payable recipient, uint256 _amount) external;
}
