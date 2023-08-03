// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/introspection/ERC165.sol";
import "./MasterToken.sol";
import "./libraries/ScaleCodec.sol";
import "./interfaces/IFAReceiver.sol";
import "./GenericApp.sol";

contract FAApp is ERC165, GenericApp, IFAReceiver {
    using ScaleCodec for uint256;
    using SafeERC20 for IERC20;

    mapping(address => AssetType) public tokens;

    bytes2 constant MINT_CALL = 0x6500;
    bytes2 constant REGISTER_ASSET_CALL = 0x6501;

    event Locked(
        address token,
        address sender,
        bytes32 recipient,
        uint256 amount,
        AssetType tokenType
    );

    event Unlocked(
        address token,
        bytes32 sender,
        address recipient,
        uint256 amount,
        AssetType tokenType
    );

    event MigratedAssets(address contractAddress);

    constructor(
        address _inbound,
        address _outbound, // an address of an IOutboundChannel contract
        address[] memory evmAssets,
        address[] memory soraAssets
    ) GenericApp(_inbound, _outbound) {
        for (uint256 i = 0; i < evmAssets.length; i++) {
            tokens[evmAssets[i]] = AssetType.Evm;
        }
        for (uint256 i = 0; i < soraAssets.length; i++) {
            tokens[soraAssets[i]] = AssetType.Sora;
        }
    }

    function supportsInterface(bytes4 interfaceId) public view virtual override(AccessControl, ERC165) returns (bool) {
        return interfaceId == type(IFAReceiver).interfaceId || super.supportsInterface(interfaceId);
    }

    function lock(address token, bytes32 recipient, uint256 amount) external {
        AssetType asset = tokens[token];
        require(amount > 0, "Must lock a positive amount");
        uint256 transferredAmount;
        if (asset == AssetType.Evm) {
            uint256 beforeBalance = IERC20(token).balanceOf(address(this));
            IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
            transferredAmount =
                IERC20(token).balanceOf(address(this)) -
                beforeBalance;
        } else if (asset == AssetType.Sora) {
            MasterToken(token).burnFrom(msg.sender, amount);
            transferredAmount = amount;
        } else {
            revert("Unregistered asset type");
        }

        emit Locked(
            token,
            msg.sender,
            recipient,
            transferredAmount,
            asset
        );

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
        uint256 amount
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        AssetType asset = tokens[token];
        require(
            recipient != address(0x0),
            "Recipient must not be a zero address"
        );
        require(amount > 0, "Must unlock a positive amount");

        if (asset == AssetType.Evm) {
            IERC20(token).safeTransfer(recipient, amount);
        } else if (asset == AssetType.Sora) {
            MasterToken(token).mintTokens(msg.sender, amount);
        } else {
            revert("Unregistered asset type");
        }
        emit Unlocked(
            token,
            sender,
            recipient,
            amount,
            asset 
        );
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
        address tokenInstance = address(
            new MasterToken(name, symbol, address(this), 0, sidechainAssetId)
        );
        tokens[tokenInstance] = AssetType.Sora;
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
    function registerAssetCall(
        address token,
        bytes32 assetId
    ) private pure returns (bytes memory) {
        return abi.encodePacked(REGISTER_ASSET_CALL, assetId, token);
    }

    /**
     * @dev Adds a new token to the bridge whitelist.
     * @param token token address
     * @param assetType type of the token
     */
    function addTokenToWhitelist(
        address token,
        AssetType assetType
    ) external onlyRole(INBOUND_CHANNEL_ROLE) {
        require(tokens[token] == AssetType.Unregistered, "Token is already registered");
        tokens[token] = assetType;
    }

    /**
     * @dev Removes a token from the bridge whitelist.
     * @param token token address
     */
    function removeTokenFromWhitelist(
        address token
    ) external onlyRole(INBOUND_CHANNEL_ROLE) {
        require(tokens[token] != AssetType.Unregistered, "Token is not registered");
        tokens[token] = AssetType.Unregistered;
    }

    function migrateAssets(
        address contractAddress,
        address[] calldata assets,
        AssetType[] calldata assetType
    ) external onlyRole(INBOUND_CHANNEL_ROLE) nonReentrant {
        uint256 length = assets.length;
        require(length == assetType.length, "Types length mismatch");
        require(ERC165(contractAddress).supportsInterface(type(IFAReceiver).interfaceId), "Invalid contract address");
        for (uint256 i = 0; i < length; i++) {
            if (assetType[i] == AssetType.Evm) {
                IERC20 token = IERC20(assets[i]);
                // slither-disable-next-line calls-loop
                token.safeTransfer(
                    contractAddress,
                    token.balanceOf(address(this))
                );
            } else if (assetType[i] == AssetType.Sora) {
                // slither-disable-next-line calls-loop
                MasterToken(assets[i]).transferOwnership(contractAddress);
            } else {
                revert("Unregistered asset type");
            }
        }
        emit MigratedAssets(contractAddress);
    }
}
