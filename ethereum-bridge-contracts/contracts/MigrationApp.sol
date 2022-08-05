// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "./MasterToken.sol";
import "./ScaleCodec.sol";
import "./IAssetRegister.sol";
import "./EthTokenReceiver.sol";
import "./GenericApp.sol";

contract MigrationApp is GenericApp, EthTokenReceiver {
    using ScaleCodec for uint256;
    using SafeERC20 for IERC20;

    constructor(address _inbound, OutboundChannel _outbound)
        GenericApp(_inbound, _outbound)
    {}

    event MigratedNativeErc20(address contractAddress);

    function migrateNativeErc20(
        address contractAddress,
        address[] calldata erc20nativeTokens
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        IAssetRegister app = IAssetRegister(contractAddress);
        for (uint256 i = 0; i < erc20nativeTokens.length; i++) {
            IERC20 token = IERC20(erc20nativeTokens[i]);
            token.safeTransfer(contractAddress, token.balanceOf(address(this)));
            app.registerExistingAsset(erc20nativeTokens[i]);
        }
        emit MigratedNativeErc20(contractAddress);
    }

    event MigratedEth(address contractAddress);

    function migrateEth(address contractAddress)
        public
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        EthTokenReceiver receiver = EthTokenReceiver(contractAddress);
        receiver.receivePayment{value: address(this).balance}();
        emit MigratedEth(contractAddress);
    }

    event MigratedSidechain(address contractAddress);

    function migrateSidechain(
        address contractAddress,
        address[] calldata sidechainTokens
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        IAssetRegister app = IAssetRegister(contractAddress);
        for (uint256 i = 0; i < sidechainTokens.length; i++) {
            Ownable token = Ownable(sidechainTokens[i]);
            token.transferOwnership(contractAddress);
            app.registerExistingAsset(sidechainTokens[i]);
        }
        emit MigratedSidechain(contractAddress);
    }

    function receivePayment() external payable override {}
}
