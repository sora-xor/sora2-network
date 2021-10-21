package parachain

import (
	"context"
	"encoding/hex"
	"errors"
	"math/big"
	"strings"

	"golang.org/x/sync/errgroup"

	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"

	geth "github.com/ethereum/go-ethereum"
	"github.com/snowfork/snowbridge/relayer/chain/ethereum"
	"github.com/snowfork/snowbridge/relayer/chain/relaychain"
	"github.com/snowfork/snowbridge/relayer/contracts/basic"
	"github.com/snowfork/snowbridge/relayer/contracts/incentivized"
	"github.com/snowfork/snowbridge/relayer/crypto/keccak"

	gsrpcTypes "github.com/vovac12/go-substrate-rpc-client/v3/types"

	log "github.com/sirupsen/logrus"
)

type EthereumChannelWriter struct {
	config                     *SinkConfig
	conn                       *ethereum.Connection
	basicInboundChannel        *basic.BasicInboundChannel
	incentivizedInboundChannel *incentivized.IncentivizedInboundChannel
	messagePackages            <-chan MessagePackage
}

func NewEthereumChannelWriter(
	config *SinkConfig,
	conn *ethereum.Connection,
	messagePackages <-chan MessagePackage,
) (*EthereumChannelWriter, error) {
	return &EthereumChannelWriter{
		config:                     config,
		conn:                       conn,
		basicInboundChannel:        nil,
		incentivizedInboundChannel: nil,
		messagePackages:            messagePackages,
	}, nil
}

func (wr *EthereumChannelWriter) Start(ctx context.Context, eg *errgroup.Group) error {
	var address common.Address

	address = common.HexToAddress(wr.config.Contracts.BasicInboundChannel)
	basic, err := basic.NewBasicInboundChannel(address, wr.conn.GetClient())
	if err != nil {
		return err
	}
	wr.basicInboundChannel = basic

	address = common.HexToAddress(wr.config.Contracts.IncentivizedInboundChannel)
	incentivized, err := incentivized.NewIncentivizedInboundChannel(address, wr.conn.GetClient())
	if err != nil {
		return err
	}
	wr.incentivizedInboundChannel = incentivized

	eg.Go(func() error {
		return wr.writeMessagesLoop(ctx)
	})

	return nil
}

func (wr *EthereumChannelWriter) makeTxOpts(ctx context.Context) *bind.TransactOpts {
	chainID := wr.conn.ChainID()
	keypair := wr.conn.GetKP()

	options := bind.TransactOpts{
		From: keypair.CommonAddress(),
		Signer: func(_ common.Address, tx *types.Transaction) (*types.Transaction, error) {
			return types.SignTx(tx, types.NewLondonSigner(chainID), keypair.PrivateKey())
		},
		Context: ctx,
	}

	if wr.config.Ethereum.GasFeeCap > 0 {
		fee := big.NewInt(0)
		fee.SetUint64(wr.config.Ethereum.GasFeeCap)
		options.GasFeeCap = fee
	}

	if wr.config.Ethereum.GasTipCap > 0 {
		tip := big.NewInt(0)
		tip.SetUint64(wr.config.Ethereum.GasTipCap)
		options.GasTipCap = tip
	}

	if wr.config.Ethereum.GasLimit > 0 {
		options.GasLimit = wr.config.Ethereum.GasLimit
	}

	return &options
}

func (wr *EthereumChannelWriter) writeMessagesLoop(ctx context.Context) error {
	options := wr.makeTxOpts(ctx)
	for {
		select {
		case <-ctx.Done():
			log.WithField("reason", ctx.Err()).Info("Shutting down ethereum writer")
			// Drain messages to avoid deadlock
			for len(wr.messagePackages) > 0 {
				<-wr.messagePackages
			}
			return nil
		case messagePackage := <-wr.messagePackages:
			err := wr.WriteChannel(options, &messagePackage)
			if err != nil {
				log.WithError(err).Error("Error submitting message to ethereum")
				return err
			}
		}
	}
}

// Submit sends a SCALE-encoded message to an application deployed on the Ethereum network
func (wr *EthereumChannelWriter) WriteBasicChannel(
	options *bind.TransactOpts,
	msgPackage *MessagePackage,
	msgs []relaychain.BasicOutboundChannelMessage,
) error {
	var messages []basic.BasicInboundChannelMessage
	for _, m := range msgs {
		messages = append(messages,
			basic.BasicInboundChannelMessage{
				Target:  m.Target,
				Nonce:   m.Nonce,
				Payload: m.Payload,
			},
		)
	}

	ownParachainHeadBytes, err := gsrpcTypes.EncodeToBytes(&msgPackage.digest)
	if err != nil {
		return err
	}
	ownParachainHeadBytesString := hex.EncodeToString(ownParachainHeadBytes)
	commitmentHashString := hex.EncodeToString(msgPackage.commitmentHash[:])
	prefixSuffix := strings.Split(ownParachainHeadBytesString, commitmentHashString)
	if len(prefixSuffix) != 2 {
		return errors.New("error splitting parachain header into prefix and suffix")
	}
	prefix, err := hex.DecodeString(prefixSuffix[0])
	if err != nil {
		return err
	}
	suffix, err := hex.DecodeString(prefixSuffix[1])
	if err != nil {
		return err
	}

	beefyMMRLeafIndex := msgPackage.mmrProof.Proof.LeafIndex
	beefyMMRLeafCount := msgPackage.mmrProof.Proof.LeafCount
	var beefyMMRProof [][32]byte
	for _, item := range msgPackage.mmrProof.Proof.Items {
		beefyMMRProof = append(beefyMMRProof, [32]byte(item))
	}

	beefyMMRLeafBytes, err := gsrpcTypes.EncodeToBytes(msgPackage.mmrProof.Leaf)
	if err != nil {
		return err
	}
	beefyMMRLeafString := hex.EncodeToString(beefyMMRLeafBytes)
	digestHashString := hex.EncodeToString(msgPackage.mmrProof.Leaf.DigestHash[:])
	beefyMMRLeafStringPartial := strings.TrimSuffix(beefyMMRLeafString, digestHashString)
	if beefyMMRLeafString == beefyMMRLeafStringPartial {
		return errors.New("invalid leaf")
	}
	beefyMMRLeafBytesPartial, err := hex.DecodeString(beefyMMRLeafStringPartial)
	if err != nil {
		return err
	}
	log.WithField("partialLeaf", beefyMMRLeafBytesPartial)

	err = wr.logBasicTx(messages, int64(beefyMMRLeafIndex), beefyMMRProof,
		msgPackage.mmrProof.Leaf,
		msgPackage.commitmentHash, msgPackage.mmrRootHash,
	)
	if err != nil {
		log.WithError(err).Error("Failed to log transaction input")
		return err
	}

	leafBytes := basic.BasicInboundChannelLeafBytes{
		DigestPrefix: prefix,
		DigestSuffix: suffix,
		LeafPrefix:   beefyMMRLeafBytesPartial,
	}
	// Pack the input, call and unpack the results
	abi, err := basic.BasicInboundChannelMetaData.GetAbi()
	input, err := abi.Pack(
		"submit", messages,
		leafBytes,
		big.NewInt(int64(beefyMMRLeafIndex)),
		big.NewInt(int64(beefyMMRLeafCount)),
		beefyMMRProof,
	)
	if err != nil {
		return err
	}
	address := common.HexToAddress(wr.config.Contracts.BasicInboundChannel)
	callMsg := geth.CallMsg{From: options.From, To: &address, Data: input}
	estimatedGas, err := wr.conn.GetClient().EstimateGas(options.Context, callMsg)
	estimatedCost := (estimatedGas * 4000 * 50) / 1000000000
	log.WithField("estimatedGas", estimatedGas).WithField("estimatedCost", estimatedCost).WithError(err).Info("Estimated gas basic")
	rawCaller := basic.BasicInboundChannelCallerRaw{Contract: &wr.basicInboundChannel.BasicInboundChannelCaller}
	callResult := make([]interface{}, 0)
	err = rawCaller.Call(&bind.CallOpts{Context: options.Context, From: options.From, Pending: false}, &callResult,
		"submit", messages,
		leafBytes,
		big.NewInt(int64(beefyMMRLeafIndex)),
		big.NewInt(int64(beefyMMRLeafCount)),
		beefyMMRProof,
	)
	log.WithFields(log.Fields{"error": err, "result": callResult}).Info("Test transaction")

	tx, err := wr.basicInboundChannel.Submit(options, messages,
		leafBytes,
		big.NewInt(int64(beefyMMRLeafIndex)), big.NewInt(int64(beefyMMRLeafCount)), beefyMMRProof)
	if err != nil {
		log.WithError(err).Error("Failed to submit transaction")
		return err
	}

	log.WithFields(log.Fields{
		"txHash":  tx.Hash().Hex(),
		"channel": "Basic",
	}).Info("Transaction submitted")

	return nil
}

func (wr *EthereumChannelWriter) WriteIncentivizedChannel(
	options *bind.TransactOpts,
	msgPackage *MessagePackage,
	msgs []relaychain.IncentivizedOutboundChannelMessage,
) error {
	var messages []incentivized.IncentivizedInboundChannelMessage
	for _, m := range msgs {
		messages = append(messages,
			incentivized.IncentivizedInboundChannelMessage{
				Target:  m.Target,
				Nonce:   m.Nonce,
				Fee:     m.Fee.Int,
				Payload: m.Payload,
			},
		)
	}
	ownParachainHeadBytes, err := gsrpcTypes.EncodeToBytes(&msgPackage.digest)
	if err != nil {
		return err
	}
	computedDigestHash := keccak.New().Hash(ownParachainHeadBytes)
	computedDigestHashString := hex.EncodeToString(computedDigestHash)
	ownParachainHeadBytesString := hex.EncodeToString(ownParachainHeadBytes)
	commitmentHashString := hex.EncodeToString(msgPackage.commitmentHash[:])
	prefixSuffix := strings.Split(ownParachainHeadBytesString, commitmentHashString)
	if len(prefixSuffix) != 2 {
		return errors.New("error splitting parachain header into prefix and suffix")
	}
	prefix, err := hex.DecodeString(prefixSuffix[0])
	if err != nil {
		return err
	}
	suffix, err := hex.DecodeString(prefixSuffix[1])
	if err != nil {
		return err
	}

	beefyMMRLeafIndex := msgPackage.mmrProof.Proof.LeafIndex
	beefyMMRLeafCount := msgPackage.mmrProof.Proof.LeafCount
	var beefyMMRProof [][32]byte
	for _, item := range msgPackage.mmrProof.Proof.Items {
		beefyMMRProof = append(beefyMMRProof, [32]byte(item))
	}

	beefyMMRLeafBytes, err := gsrpcTypes.EncodeToBytes(msgPackage.mmrProof.Leaf)
	if err != nil {
		return err
	}
	beefyMMRLeafString := hex.EncodeToString(beefyMMRLeafBytes)
	digestHashString := hex.EncodeToString(msgPackage.mmrProof.Leaf.DigestHash[:])
	beefyMMRLeafStringPartial := strings.TrimSuffix(beefyMMRLeafString, digestHashString)
	if beefyMMRLeafString == beefyMMRLeafStringPartial {
		return errors.New("invalid leaf")
	}
	log.WithField("leafString", beefyMMRLeafString).WithField("digest", digestHashString).WithField("computed", computedDigestHashString).Info("Leaf encoded")
	beefyMMRLeafBytesPartial, err := hex.DecodeString(beefyMMRLeafStringPartial)
	if err != nil {
		return err
	}
	log.WithField("partialLeaf", beefyMMRLeafBytesPartial)

	leafBytes := incentivized.IncentivizedInboundChannelLeafBytes{
		DigestPrefix: prefix,
		DigestSuffix: suffix,
		LeafPrefix:   beefyMMRLeafBytesPartial,
	}

	err = wr.logIncentivizedTx(messages, int64(beefyMMRLeafIndex), beefyMMRProof,
		msgPackage.mmrProof.Leaf,
		msgPackage.commitmentHash, msgPackage.mmrRootHash,
	)
	if err != nil {
		log.WithError(err).Error("Failed to log transaction input")
		return err
	}
	// Pack the input, call and unpack the results
	abi, err := incentivized.IncentivizedInboundChannelMetaData.GetAbi()
	input, err := abi.Pack(
		"submit", messages,
		leafBytes,
		big.NewInt(int64(beefyMMRLeafIndex)),
		big.NewInt(int64(beefyMMRLeafCount)),
		beefyMMRProof,
	)
	if err != nil {
		return err
	}
	address := common.HexToAddress(wr.config.Contracts.IncentivizedInboundChannel)
	callMsg := geth.CallMsg{From: options.From, To: &address, Data: input}
	estimatedGas, err := wr.conn.GetClient().EstimateGas(options.Context, callMsg)
	estimatedCost := (estimatedGas * 4000 * 50) / 1000000000
	log.WithField("estimatedGas", estimatedGas).WithField("estimatedCost", estimatedCost).WithError(err).Info("Estimated gas incentivized")
	rawCaller := incentivized.IncentivizedInboundChannelCallerRaw{Contract: &wr.incentivizedInboundChannel.IncentivizedInboundChannelCaller}
	callResult := make([]interface{}, 0)
	err = rawCaller.Call(&bind.CallOpts{Context: options.Context, From: options.From, Pending: false}, &callResult,
		"submit", messages,
		leafBytes,
		big.NewInt(int64(beefyMMRLeafIndex)),
		big.NewInt(int64(beefyMMRLeafCount)),
		beefyMMRProof,
	)
	log.WithFields(log.Fields{"error": err, "result": callResult}).Info("Test transaction")

	tx, err := wr.incentivizedInboundChannel.Submit(options, messages,
		leafBytes,
		big.NewInt(int64(beefyMMRLeafIndex)),
		big.NewInt(int64(beefyMMRLeafCount)),
		beefyMMRProof,
	)
	if err != nil {
		log.WithError(err).Error("Failed to submit transaction")
		return err
	}

	log.WithFields(log.Fields{
		"txHash":  tx.Hash().Hex(),
		"channel": "Incentivized",
	}).Info("Transaction submitted")

	return nil
}

func (wr *EthereumChannelWriter) WriteChannel(
	options *bind.TransactOpts,
	msg *MessagePackage,
) error {
	if msg.channelID.IsBasic {
		var outboundMessages []relaychain.BasicOutboundChannelMessage
		err := gsrpcTypes.DecodeFromBytes(msg.commitmentData, &outboundMessages)
		if err != nil {
			log.WithError(err).Error("Failed to decode commitment messages")
			return err
		}
		err = wr.WriteBasicChannel(options, msg, outboundMessages)
		if err != nil {
			log.WithError(err).Error("Failed to write basic channel")
			return err
		}

	}
	if msg.channelID.IsIncentivized {
		var outboundMessages []relaychain.IncentivizedOutboundChannelMessage
		err := gsrpcTypes.DecodeFromBytes(msg.commitmentData, &outboundMessages)
		if err != nil {
			log.WithError(err).Error("Failed to decode commitment messages")
			return err
		}
		err = wr.WriteIncentivizedChannel(options, msg, outboundMessages)
		if err != nil {
			log.WithError(err).Error("Failed to write incentivized channel")
			return err
		}
	}

	return nil
}
