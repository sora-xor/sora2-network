package relaychain

import "github.com/vovac12/go-substrate-rpc-client/v3/types"

type BasicOutboundChannelMessage struct {
	NetworkId uint64
	Channel   types.H160
	Target    [20]byte
	Nonce     uint64
	Payload   []byte
}

type IncentivizedOutboundChannelMessage struct {
	NetworkId uint64
	Channel   types.H160
	Target    [20]byte
	Nonce     uint64
	Fee       types.U256
	Payload   []byte
}
