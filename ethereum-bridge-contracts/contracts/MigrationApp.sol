// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "./MasterToken.sol";
import "./libraries/ScaleCodec.sol";
import "./interfaces/IAssetRegister.sol";
import "./interfaces/IEthTokenReceiver.sol";
import "./GenericApp.sol";

/** 
* @dev The contract was analyzed using Slither static analysis framework. All recommendations have been taken 
* into account and some detectors have been disabled at developers' discretion using `slither-disable-next-line`. 
*/
contract MigrationApp is GenericApp, IEthTokenReceiver, ReentrancyGuard {
    using ScaleCodec for uint256;
    using SafeERC20 for IERC20;

    constructor(address _inbound, address _outbound)
        GenericApp(_inbound, _outbound)
    {}

    /// Events
    event MigratedNativeErc20(address contractAddress);
    event MigratedEth(address contractAddress);
    event MigratedSidechain(address contractAddress);

    function migrateNativeErc20(
        address contractAddress,
        address[] calldata erc20nativeTokens
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        IAssetRegister app = IAssetRegister(contractAddress);
        uint256 length = erc20nativeTokens.length; 
        for (uint256 i = 0; i < length; i++) {
            IERC20 token = IERC20(erc20nativeTokens[i]);
            // slither-disable-next-line calls-loop
            token.safeTransfer(contractAddress, token.balanceOf(address(this)));
            // slither-disable-next-line calls-loop
            app.addTokenToWhitelist(erc20nativeTokens[i]);
        }
        emit MigratedNativeErc20(contractAddress);
    }

    function migrateEth(address contractAddress)
        external
        onlyRole(INBOUND_CHANNEL_ROLE)
        nonReentrant
    {
        IEthTokenReceiver receiver = IEthTokenReceiver(contractAddress);
        // slither-disable-next-line arbitrary-send
        receiver.receivePayment{value: address(this).balance}();
        emit MigratedEth(contractAddress);
    }

    function migrateSidechain(
        address contractAddress,
        address[] calldata sidechainTokens
    ) external onlyRole(INBOUND_CHANNEL_ROLE) {
        IAssetRegister app = IAssetRegister(contractAddress);
        uint256 length = sidechainTokens.length; 
        for (uint256 i = 0; i < length; i++) {
            Ownable token = Ownable(sidechainTokens[i]);
            // slither-disable-next-line calls-loop
            token.transferOwnership(contractAddress);
            // slither-disable-next-line calls-loop
            app.addTokenToWhitelist(sidechainTokens[i]);
        }
        emit MigratedSidechain(contractAddress);
    }

    function receivePayment() external payable override {}
}
