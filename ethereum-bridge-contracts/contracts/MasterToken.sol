// SPDX-License-Identifier: Apache License 2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";

contract MasterToken is ERC20Burnable, Ownable {
    bytes32 public immutable sidechainAssetId;

    /**
     * @dev Constructor that gives the specified address all of existing tokens.
     */
    constructor(
        string memory name_,
        string memory symbol_,
        address beneficiary,
        uint256 supply,
        bytes32 sideChainAssetId
    ) ERC20(name_, symbol_) {
        sidechainAssetId = sideChainAssetId;
        _mint(beneficiary, supply);
    }

    fallback() external {
        revert();
    }

    function mintTokens(address beneficiary, uint256 amount) external onlyOwner {
        _mint(beneficiary, amount);
    }
}
