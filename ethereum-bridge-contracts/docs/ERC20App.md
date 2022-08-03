## `ERC20App`






### `constructor(address inbound, contract OutboundChannel _outbound, address migrationApp)` (public)





### `lock(address _token, bytes32 _recipient, uint256 _amount)` (public)





### `unlock(address _token, bytes32 _sender, address _recipient, uint256 _amount)` (public)





### `registerAsset(address token)` (public)

Add new token from sidechain to the bridge white list.





### `registerExistingAsset(address token)` (public)






### `Locked(address token, address sender, bytes32 recipient, uint256 amount)`





### `Unlocked(address token, bytes32 sender, address recipient, uint256 amount)`





