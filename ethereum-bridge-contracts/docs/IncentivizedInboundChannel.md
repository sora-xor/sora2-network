## `IncentivizedInboundChannel`






### `constructor(contract BeefyLightClient _beefyLightClient)` (public)





### `initialize(address _configUpdater, address _rewardSource)` (external)





### `submit(struct IncentivizedInboundChannel.Message[] _messages, struct IncentivizedInboundChannel.LeafBytes _leafBytes, struct SimplifiedMMRProof proof)` (public)





### `verifyMerkleLeaf(struct IncentivizedInboundChannel.Message[] _messages, struct IncentivizedInboundChannel.LeafBytes _leafBytes, struct SimplifiedMMRProof proof)` (internal)





### `processMessages(address payable _relayer, struct IncentivizedInboundChannel.Message[] _messages)` (internal)






### `MessageDispatched(uint64 nonce, bool result)`





