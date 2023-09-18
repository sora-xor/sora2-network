// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

interface IFAReceiver {
    error InvalidRecipient();
    error InvalidAmount();
    error InvalidContract();
    error UnregisteredAsset();
    error RegisteredAsset();
    error AssetLengthMismatch();

    enum AssetType {
        Unregistered,
        Evm,
        Sora
    }

    function unlock(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount
    ) external;

    function addTokenToWhitelist(address, AssetType) external;

    function migrateAssets(
        address contractAddress,
        address[] calldata assets,
        AssetType[] calldata assetType
    ) external;
}
