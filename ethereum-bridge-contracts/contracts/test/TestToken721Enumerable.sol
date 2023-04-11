// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;

import "@openzeppelin/contracts/token/ERC721/extensions/ERC721Enumerable.sol";

contract TestToken721Enumerable is ERC721Enumerable {
    constructor(string memory _name, string memory _symbol)
        ERC721(_name, _symbol)
    {}

    function mint(address to, uint256 tokenId) external {
        _mint(to, tokenId);
    }
}
