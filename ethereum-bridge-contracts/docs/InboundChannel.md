## `InboundChannel`






### `constructor(contract BeefyLightClient _beefyLightClient)` (public)





### `initialize(address _rewardSource)` (external)





### `submit(struct InboundChannel.Batch batch, struct InboundChannel.LeafBytes _leafBytes, struct SimplifiedMMRProof proof)` (public)





### `verifyMerkleLeaf(struct InboundChannel.Batch batch, struct InboundChannel.LeafBytes _leafBytes, struct SimplifiedMMRProof proof)` (internal)





### `processMessages(address payable _relayer, struct InboundChannel.Message[] _messages)` (internal)






### `MessageDispatched(uint64 nonce, bool result)`





