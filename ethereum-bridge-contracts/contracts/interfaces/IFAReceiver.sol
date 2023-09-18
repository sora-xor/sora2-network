// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

interface IFAReceiver {
    enum AssetType {
        Unregistered,
        Evm,
        Sora
    }
    
    function migrateAssets(
        address contractAddress,
        address[] calldata assets,
        AssetType[] calldata assetType
    ) external;
}
