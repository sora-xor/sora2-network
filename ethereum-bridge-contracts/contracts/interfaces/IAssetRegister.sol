// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

interface IAssetRegister {
    enum AssetType {
        Unregistered,
        Evm,
        Sora
    }
    function addTokenToWhitelist(address, AssetType) external;
}
