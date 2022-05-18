// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

// Something that can reward a relayer
interface RewardSource {
    function reward(address payable feePayer, uint256 _amount) external;
}
