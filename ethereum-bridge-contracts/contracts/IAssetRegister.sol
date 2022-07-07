pragma solidity =0.8.13;

// SPDX-License-Identifier: MIT

interface IAssetRegister {
    function registerExistingAsset(address token) external;
}
