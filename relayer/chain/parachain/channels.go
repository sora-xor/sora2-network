package parachain

import "github.com/vovac12/go-substrate-rpc-client/v3/types"

type BasicOutboundChannelMessage struct {
	Target  [20]byte
	Nonce   uint64
	Payload []byte
}

type IncentivizedOutboundChannelMessage struct {
	Target  [20]byte
	Nonce   uint64
	Fee     types.U256
	Payload []byte
}
