// SPDX-License-Identifier: Apache-2.0
pragma solidity =0.8.13;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "./MasterToken.sol";
import "./ScaleCodec.sol";
import "./IAssetRegister.sol";
import "./GenericApp.sol";

contract SidechainApp is GenericApp, IAssetRegister {
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
        address _inbound,
        OutboundChannel _outbound,
        address migrationApp
    ) GenericApp(_inbound, _outbound) {
        _setupRole(INBOUND_CHANNEL_ROLE, migrationApp);
    }

    function lock(
        address _token,
        bytes32 _recipient,
        uint256 _amount
    ) public {
        require(tokens[_token], "Token is not registered");

        ERC20Burnable mtoken = ERC20Burnable(_token);
        mtoken.burnFrom(msg.sender, _amount);
        emit Burned(_token, msg.sender, _recipient, _amount);

        bytes memory call = mintCall(_token, msg.sender, _recipient, _amount);

        outbound.submit(msg.sender, call);
    }

    function unlock(
        address _token,
        bytes32 _sender,
        address _recipient,
        uint256 _amount
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
        require(tokens[_token], "Token is not registered");

        MasterToken tokenInstance = MasterToken(_token);
        tokenInstance.mintTokens(_recipient, _amount);
        emit Minted(_token, _sender, _recipient, _amount);
    }

    // SCALE-encode payload
    function mintCall(
        address _token,
        address _sender,
        bytes32 _recipient,
        uint256 _amount
    ) private pure returns (bytes memory) {
        return
            abi.encodePacked(
                MINT_CALL,
                _token,
                _sender,
                _recipient,
                _amount.encode256()
            );
    }

    // SCALE-encode payload
    function registerAssetCall(address _token, bytes32 _asset_id)
        private
        pure
        returns (bytes memory)
    {
        return abi.encodePacked(REGISTER_ASSET_CALL, _asset_id, _token);
    }

    /**
     * Add new token from sidechain to the bridge white list.
     *
     * @param name token title
     * @param symbol token symbol
     * @param sidechainAssetId token id on the sidechain
     */
    function registerAsset(
        string memory name,
        string memory symbol,
        bytes32 sidechainAssetId
    ) public onlyRole(INBOUND_CHANNEL_ROLE) {
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

    function registerExistingAsset(address token)
        public
        override
        onlyRole(INBOUND_CHANNEL_ROLE)
    {
        tokens[token] = true;
    }
}
