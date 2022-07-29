## `BasicOutboundChannel`






### `initialize(address[] configUpdaters, address _principal, address[] defaultOperators)` (external)





### `authorizeDefaultOperator(address operator)` (external)





### `revokeDefaultOperator(address operator)` (external)





### `setPrincipal(address _principal)` (external)





### `submit(address _origin, bytes _payload)` (external)



Sends a message across the channel

Submission is a privileged action involving two parties: The operator and the origin.
Apps (aka operators) need to be authorized by the `origin` to submit messages via this channel.

Furthermore, this channel restricts the origin to a single account, that of the principal.
In essence this ensures that only the principal account can send messages via this channel.

For pre-production testing, the restriction to the principal account can be bypassed by using
`setPrincipal` to set the principal to the address 0x0000000000000000000000000000000000000042.

### `fee() → uint256` (external)






### `Message(address source, uint64 nonce, bytes payload)`





