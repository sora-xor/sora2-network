## `OutboundChannel`






### `initialize(address[] configUpdaters, address[] defaultOperators, uint256 initial_fee)` (external)





### `setFee(uint256 _amount)` (external)





### `authorizeDefaultOperator(address operator)` (external)





### `revokeDefaultOperator(address operator)` (external)





### `submit(address feePayer, bytes payload)` (external)



Sends a message across the channel

### `fee() → uint256` (external)






### `Message(address source, uint64 nonce, uint256 fee, bytes payload)`





### `FeeChanged(uint256 oldFee, uint256 newFee)`





