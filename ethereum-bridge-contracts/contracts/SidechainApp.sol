// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "./MasterToken.sol";
import "./libraries/ScaleCodec.sol";
import "./interfaces/IAssetRegister.sol";
import "./GenericApp.sol";

/** 
* @dev The contract was analyzed using Slither static analysis framework. All recommendations have been taken 
* into account and some detectors have been disabled at developers' discretion using `slither-disable-next-line`. 
*/
contract SidechainApp is GenericApp, IAssetRegister, ReentrancyGuard {
    using ScaleCodec for uint256;

    mapping(address => bool) public tokens;

    bytes2 constant MINT_CALL = 0x6500;
    bytes2 constant REGISTER_ASSET_CALL = 0x6501;

    event Burned(
        address token,
        address sender,
        bytes32 recipient,
        uint256 amount
    );

    event Minted(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount
    );

    constructor(
        address inboundChannel,
        address outboundChannel,
        address migrationApp
    ) GenericApp(inboundChannel, outboundChannel) {
        _setupRole(INBOUND_CHANNEL_ROLE, migrationApp);
    }

    function lock(
        address token,
        bytes32 recipient,
        uint256 amount
    ) external nonReentrant {
        require(tokens[token], "Token is not registered");

        ERC20Burnable mtoken = ERC20Burnable(token);
        mtoken.burnFrom(msg.sender, amount);
        emit Burned(token, msg.sender, recipient, amount);

        bytes memory call = mintCall(token, msg.sender, recipient, amount);
        outbound.submit(msg.sender, call);
    }

    function unlock(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        require(tokens[token], "Token is not registered");

        MasterToken tokenInstance = MasterToken(token);
        tokenInstance.mintTokens(recipient, amount);
        // slither-disable-next-line reentrancy-events
        emit Minted(token, sender, recipient, amount);
    }

    // SCALE-encode payload
    function mintCall(
        address token,
        address sender,
        bytes32 recipient,
        uint256 amount
    ) private pure returns (bytes memory) {
        return
            abi.encodePacked(
                MINT_CALL,
                token,
                sender,
                recipient,
                amount.encode256()
            );
    }

    // SCALE-encode payload
    function registerAssetCall(address token, bytes32 assetId)
        private
        pure
        returns (bytes memory)
    {
        return abi.encodePacked(REGISTER_ASSET_CALL, assetId, token);
    }

    /**
     * Add new token from sidechain to the bridge white list.
     * @dev Should be called from a contract or an instance (INBOUND_CHANNEL_ROLE) which performs necessary checks.
     * No extra checks are applied to the token deploying process.
     * @param name token title
     * @param symbol token symbol
     * @param sidechainAssetId token id on the sidechain
     */
    function createNewToken(
        string memory name,
        string memory symbol,
        bytes32 sidechainAssetId
    ) external onlyRole(INBOUND_CHANNEL_ROLE) {
        // Create new instance of the token
        MasterToken tokenInstance = new MasterToken(
            name,
            symbol,
            address(this),
            0,
            sidechainAssetId
        );
        address tokenAddress = address(tokenInstance);
        tokens[tokenAddress] = true;

        bytes memory call = registerAssetCall(tokenAddress, sidechainAssetId);

        outbound.submit(msg.sender, call);
    }

    function addTokenToWhitelist(address token)
        external
        override
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        require(!tokens[token], "Token is already registered");
        tokens[token] = true;
    }
}
