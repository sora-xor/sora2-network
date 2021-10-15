package parachain

import (
	"context"

	"golang.org/x/sync/errgroup"

	"github.com/snowfork/snowbridge/relayer/chain/ethereum"
	"github.com/snowfork/snowbridge/relayer/chain/relaychain"
	"github.com/snowfork/snowbridge/relayer/crypto/secp256k1"

	log "github.com/sirupsen/logrus"
)

type Relay struct {
	config                *Config
	relaychainConn        *relaychain.Connection
	ethereumConn          *ethereum.Connection
	ethereumChannelWriter *EthereumChannelWriter
	beefyListener         *BeefyListener
}

func NewRelay(config *Config, keypair *secp256k1.Keypair) (*Relay, error) {
	log.Info("Creating worker")

	relaychainConn := relaychain.NewConnection(config.Source.Substrate.Endpoint)

	// TODO: This is used by both the source & sink. They should use separate connections
	ethereumConn := ethereum.NewConnection(config.Sink.Ethereum.Endpoint, keypair)

	// channel for messages from beefy listener to ethereum writer
	var messagePackages = make(chan MessagePackage, 1)

	ethereumChannelWriter, err := NewEthereumChannelWriter(
		&config.Sink,
		ethereumConn,
		messagePackages,
	)
	if err != nil {
		return nil, err
	}

	beefyListener := NewBeefyListener(
		&config.Source,
		ethereumConn,
		relaychainConn,
		messagePackages,
	)

	return &Relay{
		config:                config,
		relaychainConn:        relaychainConn,
		ethereumConn:          ethereumConn,
		ethereumChannelWriter: ethereumChannelWriter,
		beefyListener:         beefyListener,
	}, nil
}

func (relay *Relay) Start(ctx context.Context, eg *errgroup.Group) error {
	err := relay.ethereumConn.Connect(ctx)
	if err != nil {
		return err
	}

	err = relay.relaychainConn.Connect(ctx)
	if err != nil {
		return err
	}

	log.Info("Starting beefy listener")
	err = relay.beefyListener.Start(ctx, eg)
	if err != nil {
		return err
	}

	log.Info("Starting ethereum writer")
	err = relay.ethereumChannelWriter.Start(ctx, eg)
	if err != nil {
		return err
	}

	return nil
}
