## `SidechainApp`






### `constructor(address inbound, contract OutboundChannel _outbound, address migrationApp)` (public)





### `lock(address _token, bytes32 _recipient, uint256 _amount)` (public)





### `unlock(address _token, bytes32 _sender, address _recipient, uint256 _amount)` (public)





### `registerAsset(string name, string symbol, bytes32 sidechainAssetId)` (public)

Add new token from sidechain to the bridge white list.





### `registerExistingAsset(address token)` (public)






### `Burned(address token, address sender, bytes32 recipient, uint256 amount)`





### `Minted(address token, bytes32 sender, address recipient, uint256 amount)`





