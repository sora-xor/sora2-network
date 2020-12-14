pragma solidity ^0.7.4;
// "SPDX-License-Identifier: Apache License 2.0"

contract NftMigration {
    
    address public owner;
    address public nftCreator = 0x3482549fCa7511267C9Ef7089507c0F16eA1dcC1;
    IERC1155 soramotoContract = IERC1155(0xd07dc4262BCDbf85190C01c996b4C06a461d2430); 
    mapping (uint => bool) public acceptableNft;
    
    constructor() {
        owner = msg.sender;
        acceptableNft[6895] = true;
        acceptableNft[88849] = true;
        acceptableNft[77235] = true;
        acceptableNft[24403] = true;
        acceptableNft[6929] = true;
        acceptableNft[30297] = true;
        acceptableNft[12277] = true;
    }
    
    event Deposit(
        bytes32 substrateAddress, 
        uint256[] tokenIds, 
        uint256[] values);
    
    function deposit(
        bytes32 substrateAddress, 
        uint256[] calldata tokenIds, 
        uint256[] calldata values,
        bytes calldata data)
        public {
        if(msg.sender != owner && msg.sender != nftCreator) {
            require(soramotoContract.isApprovedForAll(msg.sender, address(this)), "Tokens are not approved");
            for (uint i=0; i<tokenIds.length; i++) {
                require(acceptableNft[tokenIds[i]], "One of NFT Id is not acceptable");
            }
            soramotoContract.safeBatchTransferFrom(
            msg.sender, 
            address(this), 
            tokenIds, 
            values,
            data);
            
            emit Deposit(
                substrateAddress,
                tokenIds, 
                values
            );
            }
        }
        
/*    function test() public {
        bytes32 substrateAddress = keccak256(abi.encode("Some test value"));

        uint256[] memory tokenIds = new uint256[](3);
        tokenIds[0] = (uint(34234));
        tokenIds[1] = uint(65463);
        tokenIds[2] = uint(457567);
        
        uint256[] memory values = new uint256[](3);
        values[0] = uint(23);
        values[1] = uint(654);
        values[2] = uint(4575);
        emit Deposit(
                    substrateAddress,
                    tokenIds, 
                    values
                );
    }*/
}

interface IERC165 {
    function supportsInterface(bytes4 interfaceId) external view returns (bool);
}

interface IERC1155 is IERC165 {
    event TransferSingle(address indexed _operator, address indexed _from, address indexed _to, uint256 _id, uint256 _value);
    event TransferBatch(address indexed _operator, address indexed _from, address indexed _to, uint256[] _ids, uint256[] _values);
    event ApprovalForAll(address indexed _owner, address indexed _operator, bool _approved);
    event URI(string _value, uint256 indexed _id);
    
    function safeTransferFrom(address _from, address _to, uint256 _id, uint256 _value, bytes calldata _data) external;
    function safeBatchTransferFrom(address _from, address _to, uint256[] calldata _ids, uint256[] calldata _values, bytes calldata _data) external;
    function balanceOf(address _owner, uint256 _id) external view returns (uint256);
    function balanceOfBatch(address[] calldata _owners, uint256[] calldata _ids) external view returns (uint256[] memory);
    function setApprovalForAll(address _operator, bool _approved) external;
    function isApprovedForAll(address _owner, address _operator) external view returns (bool);
}