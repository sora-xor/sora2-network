// Copyright 2020 Snowfork
// SPDX-License-Identifier: LGPL-3.0-only

package register_bridge

import (
	"context"
	"encoding/hex"
	"fmt"
	"io/ioutil"
	"math/big"
	"os"
	"os/signal"
	"strings"
	"syscall"

	gethCommon "github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/ethclient"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"golang.org/x/sync/errgroup"

	gethTypes "github.com/ethereum/go-ethereum/core/types"
	"github.com/snowfork/snowbridge/relayer/chain/ethereum"
	"github.com/snowfork/snowbridge/relayer/chain/parachain"
	"github.com/snowfork/snowbridge/relayer/crypto/sr25519"
	"github.com/vovac12/go-substrate-rpc-client/v3/types"
)

type Format string

const (
	RustFmt Format = "rust"
	JSONFmt Format = "json"
)

var (
	configFile     string
	privateKey     string
	privateKeyFile string
)

func Command() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "register-network",
		Short:   "Retrieve a block, either specified by hash or the latest finalized block and register network with it",
		Args:    cobra.ExactArgs(0),
		Example: "snowbridge-relay dump-block",
		RunE:    GetBlockFn,
	}
	cmd.Flags().StringP("block", "b", "", "Block hash")
	cmd.Flags().StringP("url", "u", "", "Ethereum endpoint")
	cmd.Flags().StringP("sora", "s", "", "SORA endpoint")
	cmd.Flags().StringVar(&privateKey, "substrate.private-key", "", "Private key URI for Substrate")
	cmd.Flags().StringVar(&privateKeyFile, "substrate.private-key-file", "", "The file from which to read the private key URI")

	return cmd
}

func GetBlockFn(cmd *cobra.Command, _ []string) error {
	ctx, eg := contextAndEG()
	hashStr := cmd.Flags().Lookup("block").Value.String()
	var blockHash *gethCommon.Hash
	if len(hashStr) > 0 {
		hashBytes, err := hex.DecodeString(hashStr)
		if err != nil {
			return err
		}
		hash := gethCommon.BytesToHash(hashBytes)
		blockHash = &hash
	}

	url := cmd.Flags().Lookup("url").Value.String()
	header, chainId, err := getEthBlock(url, blockHash)
	if err != nil {
		return err
	}

	keypair, err := resolvePrivateKey(privateKey, privateKeyFile)
	if err != nil {
		return err
	}

	soraUrl := cmd.Flags().Lookup("sora").Value.String()
	paraconn := parachain.NewConnection(soraUrl, keypair.AsKeyringPair())

	err = paraconn.Connect(ctx)
	if err != nil {
		return err
	}

	pool := parachain.NewExtrinsicPool(eg, paraconn)

	header_call, err := makeHeaderImportCall(paraconn, chainId.Uint64(), header, 0)
	if err != nil {
		return err
	}

	log.WithField("chainId", chainId).Info("Register network")

	onFinalized := func(_ types.Hash) error {
		log.Info("Registered")
		return nil
	}
	err = callExtrincic(paraconn, pool, ctx, header_call, onFinalized)
	if err != nil {
		return err
	}
	<-ctx.Done()
	return ctx.Err()
}

func getEthBlock(url string, blockHash *gethCommon.Hash) (*gethTypes.Header, *big.Int, error) {
	ctx := context.Background()
	client, err := ethclient.Dial(url)
	if err != nil {
		return nil, nil, err
	}
	defer client.Close()

	chainId, err := client.ChainID(ctx)
	if err != nil {
		return nil, nil, err
	}

	var header *gethTypes.Header
	if blockHash == nil {
		header, err = client.HeaderByNumber(ctx, nil)
		if err != nil {
			return nil, nil, err
		}
	} else {
		header, err = client.HeaderByHash(ctx, *blockHash)
		if err != nil {
			return nil, nil, err
		}
	}

	return header, chainId, nil
}

func bytesAsArray64(bytes []byte) []uint64 {
	arr := make([]uint64, len(bytes))
	for i, v := range bytes {
		arr[i] = uint64(v)
	}
	return arr
}

func resolvePrivateKey(privateKey, privateKeyFile string) (*sr25519.Keypair, error) {
	var cleanedKeyURI string

	if privateKey == "" {
		if privateKeyFile == "" {
			return nil, fmt.Errorf("private key URI not supplied")
		}
		content, err := ioutil.ReadFile(privateKeyFile)
		if err != nil {
			log.Fatal(err)
		}
		cleanedKeyURI = strings.TrimSpace(string(content))
	} else {
		cleanedKeyURI = privateKey
	}

	keypair, err := sr25519.NewKeypairFromSeed(cleanedKeyURI, 42)
	if err != nil {
		return nil, fmt.Errorf("unable to parse private key URI: %w", err)
	}

	return keypair, nil
}

func contextAndEG() (context.Context, *errgroup.Group) {
	ctx, cancel := context.WithCancel(context.Background())
	eg, ctx := errgroup.WithContext(ctx)

	// Ensure clean termination upon SIGINT, SIGTERM
	eg.Go(func() error {
		notify := make(chan os.Signal, 1)
		signal.Notify(notify, syscall.SIGINT, syscall.SIGTERM)

		select {
		case <-ctx.Done():
			return ctx.Err()
		case sig := <-notify:
			log.WithField("signal", sig.String()).Info("Received signal")
			cancel()
		}

		return nil
	})
	return ctx, eg
}

func queryAccountNonce(conn *parachain.Connection) (uint32, error) {
	key, err := types.CreateStorageKey(conn.Metadata(), "System", "Account", conn.Keypair().PublicKey, nil)
	if err != nil {
		return 0, err
	}

	var accountInfo types.AccountInfo
	ok, err := conn.API().RPC.State.GetStorageLatest(key, &accountInfo)
	if err != nil {
		return 0, err
	}
	if !ok {
		return 0, fmt.Errorf("no account info found for %s", conn.Keypair().URI)
	}

	return uint32(accountInfo.Nonce), nil
}

func callExtrincic(
	conn *parachain.Connection,
	pool *parachain.ExtrinsicPool,
	ctx context.Context,
	c types.Call,
	onFinalized parachain.OnFinalized,
) error {
	nonce, err := queryAccountNonce(conn)
	if err != nil {
		return err
	}

	genesisHash, err := conn.API().RPC.Chain.GetBlockHash(0)
	if err != nil {
		return err
	}
	ext := types.NewExtrinsic(c)

	latestHash, err := conn.API().RPC.Chain.GetFinalizedHead()
	if err != nil {
		return err
	}

	latestBlock, err := conn.API().RPC.Chain.GetBlock(latestHash)
	if err != nil {
		return err
	}

	era := parachain.NewMortalEra(uint64(latestBlock.Block.Header.Number))

	rv, err := conn.API().RPC.State.GetRuntimeVersionLatest()
	if err != nil {
		return err
	}

	o := types.SignatureOptions{
		BlockHash:          latestHash,
		Era:                era,
		GenesisHash:        genesisHash,
		Nonce:              types.NewUCompactFromUInt(uint64(nonce)),
		SpecVersion:        rv.SpecVersion,
		Tip:                types.NewUCompactFromUInt(0),
		TransactionVersion: rv.TransactionVersion,
	}

	extI := ext
	err = extI.Sign(*conn.Keypair(), o)
	if err != nil {
		return err
	}
	log.WithFields(log.Fields{
		"nonce": nonce,
	}).Info("Submitting transaction")
	err = pool.WaitForSubmitAndWatch(ctx, &extI, onFinalized)
	if err != nil {
		log.WithError(err).WithField("nonce", nonce).Debug("Failed to submit extrinsic")
		return err
	}

	return nil
}

func makeHeaderImportCall(conn *parachain.Connection, chainId uint64, header *gethTypes.Header, initial_difficulty int64) (types.Call, error) {
	encodedHeader, err := ethereum.MakeHeaderData(header)

	if err != nil {
		return types.Call{}, fmt.Errorf("header is nil")
	}

	encoded_initial_difficulty := types.NewU256(*big.NewInt(int64(initial_difficulty)))

	return types.NewCall(conn.Metadata(), "EthereumLightClient.register_network", chainId, encodedHeader, encoded_initial_difficulty)
}
