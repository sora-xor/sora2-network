package parachain

import (
	"context"
	"fmt"

	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	gethTypes "github.com/ethereum/go-ethereum/core/types"
	"golang.org/x/sync/errgroup"

	"github.com/snowfork/snowbridge/relayer/chain/ethereum"
	"github.com/snowfork/snowbridge/relayer/chain/relaychain"
	"github.com/snowfork/snowbridge/relayer/contracts/beefylightclient"
	"github.com/vovac12/go-substrate-rpc-client/v3/types"

	log "github.com/sirupsen/logrus"
)

type BeefyListener struct {
	config           *SourceConfig
	ethereumConn     *ethereum.Connection
	beefyLightClient *beefylightclient.Contract
	relaychainConn   *relaychain.Connection
	messages         chan<- MessagePackage
	chainId          uint64
}

func NewBeefyListener(
	config *SourceConfig,
	ethereumConn *ethereum.Connection,
	relaychainConn *relaychain.Connection,
	messages chan<- MessagePackage,
) *BeefyListener {
	return &BeefyListener{
		config:         config,
		ethereumConn:   ethereumConn,
		relaychainConn: relaychainConn,
		messages:       messages,
	}
}

func (li *BeefyListener) Start(ctx context.Context, eg *errgroup.Group) error {
	li.chainId = li.ethereumConn.ChainID().Uint64()

	// Set up light client bridge contract
	address := common.HexToAddress(li.config.Contracts.BeefyLightClient)
	beefyLightClientContract, err := beefylightclient.NewContract(address, li.ethereumConn.GetClient())
	if err != nil {
		return err
	}
	li.beefyLightClient = beefyLightClientContract

	eg.Go(func() error {
		beefyBlockNumber, beefyBlockHash, err := li.fetchLatestBeefyBlock(ctx)
		if err != nil {
			log.WithError(err).Error("Failed to get latest relay chain block number and hash")
			return err
		}

		log.WithFields(log.Fields{
			"blockHash":   beefyBlockHash.Hex(),
			"blockNumber": beefyBlockNumber,
		}).Info("Fetched latest verified polkadot block")

		messagePackages, err := li.buildMissedMessagePackages(ctx,
			beefyBlockNumber, beefyBlockHash)
		if err != nil {
			log.WithError(err).Error("Failed to build missed message package")
			return err
		}

		log.WithField("packages", len(messagePackages)).Info("Emit message packages")
		err = li.emitMessagePackages(ctx, messagePackages)
		if err != nil {
			return err
		}

		err = li.subBeefyJustifications(ctx)
		return err
	})

	return nil
}

func (li *BeefyListener) subBeefyJustifications(ctx context.Context) error {
	log.Info("Sub justifications")
	headers := make(chan *gethTypes.Header, 5)

	sub, err := li.ethereumConn.GetClient().SubscribeNewHead(ctx, headers)
	if err != nil {
		log.WithError(err).Error("Error creating ethereum header subscription")
		return err
	}
	defer sub.Unsubscribe()

	for {
		select {
		case <-ctx.Done():
			log.WithField("reason", ctx.Err()).Info("Shutting down beefy listener")
			if li.messages != nil {
				close(li.messages)
			}
			return nil
		case err := <-sub.Err():
			log.WithError(err).Error("Error with ethereum header subscription")
			return err
		case gethheader := <-headers:
			// Query LightClientBridge contract's ContractNewMMRRoot events
			blockNumber := gethheader.Number.Uint64()
			var beefyLightClientEvents []*beefylightclient.ContractNewMMRRoot

			contractEvents, err := li.queryBeefyLightClientEvents(ctx, blockNumber, &blockNumber)
			if err != nil {
				log.WithError(err).Error("Failure fetching event logs")
				return err
			}
			beefyLightClientEvents = append(beefyLightClientEvents, contractEvents...)

			// if len(beefyLightClientEvents) > 0 {
			log.Info(fmt.Sprintf("Found %d BeefyLightClient ContractNewMMRRoot events on block %d", len(beefyLightClientEvents), blockNumber))
			// }

			err = li.processBeefyLightClientEvents(ctx, beefyLightClientEvents)
			if err != nil {
				return err
			}
		}
	}
}

// processLightClientEvents matches events to BEEFY commitment info by transaction hash
func (li *BeefyListener) processBeefyLightClientEvents(ctx context.Context, events []*beefylightclient.ContractNewMMRRoot) error {
	for _, event := range events {

		beefyBlockNumber := event.BlockNumber

		log.WithFields(log.Fields{
			"beefyBlockNumber":    beefyBlockNumber,
			"ethereumBlockNumber": event.Raw.BlockNumber,
			"ethereumTxHash":      event.Raw.TxHash.Hex(),
		}).Info("Witnessed a new MMRRoot event")

		log.WithField("beefyBlockNumber", beefyBlockNumber).Info("Getting hash for relay chain block")
		beefyBlockHash, err := li.relaychainConn.API().RPC.Chain.GetBlockHash(uint64(beefyBlockNumber))
		if err != nil {
			log.WithError(err).Error("Failed to get block hash")
			return err
		}
		log.WithField("beefyBlockHash", beefyBlockHash.Hex()).Info("Got relay chain blockhash")

		messagePackages, err := li.buildMissedMessagePackages(ctx, beefyBlockNumber, beefyBlockHash)
		if err != nil {
			log.WithError(err).Error("Failed to build missed message packages")
			return err
		}

		err = li.emitMessagePackages(ctx, messagePackages)
		if err != nil {
			return err
		}
	}
	return nil
}

func (li *BeefyListener) emitMessagePackages(ctx context.Context, packages []MessagePackage) error {
	for _, messagePackage := range packages {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case li.messages <- messagePackage:
			log.Info("Beefy Listener emitted new message package")
		}
	}

	return nil
}

// queryBeefyLightClientEvents queries ContractNewMMRRoot events from the BeefyLightClient contract
func (li *BeefyListener) queryBeefyLightClientEvents(ctx context.Context, start uint64,
	end *uint64) ([]*beefylightclient.ContractNewMMRRoot, error) {
	var events []*beefylightclient.ContractNewMMRRoot
	filterOps := bind.FilterOpts{Start: start, End: end, Context: ctx}

	iter, err := li.beefyLightClient.FilterNewMMRRoot(&filterOps)
	if err != nil {
		return nil, err
	}

	for {
		more := iter.Next()
		if !more {
			err = iter.Error()
			if err != nil {
				return nil, err
			}
			break
		}

		events = append(events, iter.Event)
	}

	return events, nil
}

// Fetch the latest verified beefy block number and hash from Ethereum
func (li *BeefyListener) fetchLatestBeefyBlock(ctx context.Context) (uint64, types.Hash, error) {
	number, err := li.beefyLightClient.LatestBeefyBlock(&bind.CallOpts{
		Pending: false,
		Context: ctx,
	})
	if err != nil {
		log.WithError(err).Error("Failed to get latest verified beefy block number from ethereum")
		return 0, types.Hash{}, err
	}

	hash, err := li.relaychainConn.API().RPC.Chain.GetBlockHash(number)
	if err != nil {
		log.WithError(err).Error("Failed to get latest relay chain block hash from relay chain")
		return 0, types.Hash{}, err
	}

	return number, hash, nil
}
