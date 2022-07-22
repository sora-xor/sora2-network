## `ERC20App`






### `constructor(struct ERC20App.Channel _basic, struct ERC20App.Channel _incentivized, address migrationApp)` (public)





### `lock(address _token, bytes32 _recipient, uint256 _amount, enum ChannelId _channelId)` (public)





### `unlock(address _token, bytes32 _sender, address _recipient, uint256 _amount)` (public)





### `registerAsset(address token)` (public)

Add new token from sidechain to the bridge white list.





### `registerExistingAsset(address token)` (public)






### `Locked(address token, address sender, bytes32 recipient, uint256 amount)`





### `Unlocked(address token, bytes32 sender, address recipient, uint256 amount)`





