// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "./MasterToken.sol";
import "./libraries/ScaleCodec.sol";
import "./interfaces/IAssetRegister.sol";
import "./GenericApp.sol";

contract FAApp is GenericApp, IAssetRegister, ReentrancyGuard {
    using ScaleCodec for uint256;
    using SafeERC20 for IERC20;

    mapping(address => bool) public tokens;

    bytes2 constant MINT_CALL = 0x6500;
    bytes2 constant REGISTER_ASSET_CALL = 0x6501;

    event Locked(
        address token,
        address sender,
        bytes32 recipient,
        uint256 amount,
        bool tokenType
    );

    event Unlocked(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount,
        bool tokenType
    );

    event MigratedNativeErc20(address contractAddress);
    event MigratedSidechain(address contractAddress);

    constructor(
        address _inbound,
        address _outbound, // an address of an IOutboundChannel contract
        address migrationApp
    ) GenericApp(_inbound, _outbound) {
        _setupRole(INBOUND_CHANNEL_ROLE, migrationApp);
    }

    function lock(
        address token,
        bytes32 recipient,
        uint256 amount,
        bool native
    ) external {
        require(tokens[token], "Token is not registered");
        require(amount > 0, "Must lock a positive amount");
        uint256 transferredAmount;
        if (native) {
            uint256 beforeBalance = IERC20(token).balanceOf(address(this));
            IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
            transferredAmount = IERC20(token).balanceOf(address(this)) - beforeBalance;
        }
        else {
            MasterToken(token).burnFrom(msg.sender, amount);
            transferredAmount = amount;
        }
        
        emit Locked(token, msg.sender, recipient, transferredAmount, native);

        bytes memory call = encodeCall(
            token,
            msg.sender,
            recipient,
            transferredAmount
        );

        outbound.submit(msg.sender, call);
    }

    function unlock(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount,
        bool native
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        require(tokens[token], "Token is not registered");
        require(
            recipient != address(0x0),
            "Recipient must not be a zero address"
        );
        require(amount > 0, "Must unlock a positive amount");

        if (native) {
            IERC20(token).safeTransfer(recipient, amount);
        }
        else {
            MasterToken(token).mintTokens(msg.sender, amount);
        }
        emit Unlocked(token, sender, recipient, amount, native);  
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
        address tokenInstance = address(new MasterToken(
            name,
            symbol,
            address(this),
            0,
            sidechainAssetId
        ));
        tokens[tokenInstance] = true;
        bytes memory call = registerAssetCall(tokenInstance, sidechainAssetId);
        outbound.submit(msg.sender, call);
    }

    // SCALE-encode payload
    function encodeCall(
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
     * @dev Adds a new token from sidechain to the bridge whitelist.
     * @param token token address
     */
    function addTokenToWhitelist(address token)
        external
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        require(!tokens[token], "Token is already registered");
        tokens[token] = true;
    }

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
}
