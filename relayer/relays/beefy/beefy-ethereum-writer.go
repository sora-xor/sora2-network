package beefy

import (
	"context"
	"encoding/hex"
	"errors"
	"fmt"
	"math/big"
	"strconv"
	"sync/atomic"
	"time"

	"golang.org/x/sync/errgroup"

	geth "github.com/ethereum/go-ethereum"
	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"

	"github.com/snowfork/snowbridge/relayer/chain/ethereum"
	"github.com/snowfork/snowbridge/relayer/contracts/beefylightclient"
	"github.com/snowfork/snowbridge/relayer/relays/beefy/store"

	log "github.com/sirupsen/logrus"
)

type BeefyEthereumWriter struct {
	config           *SinkConfig
	ethereumConn     *ethereum.Connection
	beefyDB          *store.Database
	beefyLightClient *beefylightclient.Contract
	databaseMessages chan<- store.DatabaseCmd
	beefyMessages    <-chan store.BeefyRelayInfo
	contractId       int64
}

func NewBeefyEthereumWriter(
	config *SinkConfig,
	ethereumConn *ethereum.Connection,
	beefyDB *store.Database,
	databaseMessages chan<- store.DatabaseCmd,
	beefyMessages <-chan store.BeefyRelayInfo,
) *BeefyEthereumWriter {
	return &BeefyEthereumWriter{
		config:           config,
		ethereumConn:     ethereumConn,
		beefyDB:          beefyDB,
		databaseMessages: databaseMessages,
		beefyMessages:    beefyMessages,
		contractId:       0,
	}
}

func (wr *BeefyEthereumWriter) Start(ctx context.Context, eg *errgroup.Group) error {

	address := common.HexToAddress(wr.config.Contracts.BeefyLightClient)
	beefyLightClientContract, err := beefylightclient.NewContract(address, wr.ethereumConn.GetClient())
	if err != nil {
		return err
	}
	wr.beefyLightClient = beefyLightClientContract

	eg.Go(func() error {
		err := wr.writeMessagesLoop(ctx)
		log.WithField("reason", err).Info("Shutting down ethereum writer")
		if err != nil {
			if errors.Is(err, context.Canceled) {
				return nil
			}
			return err
		}
		return nil
	})

	return nil
}

func (wr *BeefyEthereumWriter) writeMessagesLoop(ctx context.Context) error {
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case msg, ok := <-wr.beefyMessages:
			if !ok {
				return nil
			}
			switch msg.Status {
			case store.CommitmentWitnessed:
				err := wr.WriteNewSignatureCommitment(ctx, msg)
				if err != nil {
					log.WithError(err).Error("Failed to write new signature commitment")
					return err
				}
			case store.ReadyToComplete:
				err := wr.WriteCompleteSignatureCommitment(ctx, msg)
				if err != nil {
					log.WithError(err).Error("Failed to write complete signature commitment")
					return err
				}
			}
			// Rate-limit transaction sending to reduce the chance of transactions using the same pending nonce.
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(2 * time.Second):
			}
		}
	}
}

func (wr *BeefyEthereumWriter) makeTxOpts(ctx context.Context) *bind.TransactOpts {
	chainID := wr.ethereumConn.ChainID()
	keypair := wr.ethereumConn.GetKP()

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

func (wr *BeefyEthereumWriter) WriteNewSignatureCommitment(ctx context.Context, info store.BeefyRelayInfo) error {
	beefyJustification, err := info.ToBeefyJustification()
	if err != nil {
		return fmt.Errorf("error converting BeefyRelayInfo to BeefyJustification: %s", err.Error())
	}

	contract := wr.beefyLightClient
	if contract == nil {
		return fmt.Errorf("unknown contract")
	}

	signedValidators := []*big.Int{}
	for i, signature := range beefyJustification.SignedCommitment.Signatures {
		// TODO: skip over empty/missing signatures
		if signature.Value != (store.BeefySignature{}) {
			signedValidators = append(signedValidators, big.NewInt(int64(i)))
		}
	}
	numberOfValidators := big.NewInt(int64(len(beefyJustification.SignedCommitment.Signatures)))
	initialBitfield, err := contract.CreateInitialBitfield(
		&bind.CallOpts{Pending: true}, signedValidators, numberOfValidators,
	)
	if err != nil {
		log.WithError(err).Error("Failed to create initial validator bitfield")
		return err
	}

	valIndex := signedValidators[0].Int64()

	msg, err := beefyJustification.BuildNewSignatureCommitmentMessage(valIndex, initialBitfield)
	if err != nil {
		return err
	}

	options := wr.makeTxOpts(ctx)

	// Pack the input, call and unpack the results
	abi, err := beefylightclient.ContractMetaData.GetAbi()
	input, err := abi.Pack(
		"newSignatureCommitment",
		msg.CommitmentHash,
		msg.ValidatorClaimsBitfield, msg.ValidatorSignatureCommitment,
		msg.ValidatorPosition, msg.ValidatorPublicKey, msg.ValidatorPublicKeyMerkleProof)
	if err != nil {
		return err
	}
	address := common.HexToAddress(wr.config.Contracts.BeefyLightClient)
	callMsg := geth.CallMsg{From: options.From, To: &address, Data: input}
	estimatedGas, err := wr.ethereumConn.GetClient().EstimateGas(ctx, callMsg)
	estimatedCost := (estimatedGas * 4000 * 50) / 1000000000
	log.WithField("estimatedGas", estimatedGas).WithField("estimatedCost", estimatedCost).WithError(err).Info("Estimated gas newCommitment")
	rawCaller := beefylightclient.ContractCallerRaw{Contract: &contract.ContractCaller}
	callResult := make([]interface{}, 0)
	err = rawCaller.Call(&bind.CallOpts{Context: options.Context, From: options.From, Pending: false}, &callResult,
		"newSignatureCommitment",
		msg.CommitmentHash,
		msg.ValidatorClaimsBitfield, msg.ValidatorSignatureCommitment,
		msg.ValidatorPosition, msg.ValidatorPublicKey, msg.ValidatorPublicKeyMerkleProof)
	log.WithFields(log.Fields{"error": err, "result": callResult}).Info("Test transaction")
	if err != nil {
		return err
	}

	tx, err := contract.NewSignatureCommitment(options,
		msg.CommitmentHash,
		msg.ValidatorClaimsBitfield, msg.ValidatorSignatureCommitment,
		msg.ValidatorPosition, msg.ValidatorPublicKey, msg.ValidatorPublicKeyMerkleProof)
	if err != nil {
		log.WithError(err).Error("Failed to submit transaction")
		return err
	}

	var pkProofHex []string
	for _, proofItem := range msg.ValidatorPublicKeyMerkleProof {
		pkProofHex = append(pkProofHex, "0x"+hex.EncodeToString(proofItem[:]))
	}

	log.WithFields(log.Fields{
		"txHash":                            tx.Hash().Hex(),
		"msg.CommitmentHash":                "0x" + hex.EncodeToString(msg.CommitmentHash[:]),
		"msg.ValidatorSignatureCommitment":  "0x" + hex.EncodeToString(msg.ValidatorSignatureCommitment),
		"msg.ValidatorPublicKey":            msg.ValidatorPublicKey.Hex(),
		"msg.ValidatorPublicKeyMerkleProof": pkProofHex,
		"BlockNumber":                       beefyJustification.SignedCommitment.Commitment.BlockNumber,
		"gas":                               tx.Gas(),
	}).Info("New Signature Commitment transaction submitted")

	log.Info("1: Creating item in Database with status 'InitialVerificationTxSent'")
	info.Status = store.InitialVerificationTxSent
	info.InitialVerificationTxHash = tx.Hash()

	select {
	case <-ctx.Done():
		return ctx.Err()
	case wr.databaseMessages <- store.NewDatabaseCmd(&info, store.Create, nil):
	}

	return nil
}

func BitfieldToString(bitfield []*big.Int) string {
	bitfieldString := ""
	for _, bitfieldInt := range bitfield {
		bits := strconv.FormatInt(bitfieldInt.Int64(), 2)

		// add bits from this int at leftmost position
		bitfieldString = bits + bitfieldString

		// pad to 256 bits to include missing validators
		for bitsLength := len(bits); bitsLength < 256; bitsLength++ {
			bitfieldString = "0" + bitfieldString
		}
	}
	return bitfieldString
}

// WriteCompleteSignatureCommitment sends a CompleteSignatureCommitment tx to the BeefyLightClient contract
func (wr *BeefyEthereumWriter) WriteCompleteSignatureCommitment(ctx context.Context, info store.BeefyRelayInfo) error {
	beefyJustification, err := info.ToBeefyJustification()
	if err != nil {
		return fmt.Errorf("error converting BeefyRelayInfo to BeefyJustification: %s", err.Error())
	}

	ok := atomic.CompareAndSwapInt64(&wr.contractId, info.ContractID, info.ContractID+1)
	if !ok {
		log.WithFields(log.Fields{"expectedContractId": wr.contractId, "passedContractId": info.ContractID}).Error("Wrong contract id")
		return nil
	}

	contract := wr.beefyLightClient
	if contract == nil {
		return fmt.Errorf("unknown contract")
	}

	randomBitfield, err := contract.CreateRandomBitfield(
		&bind.CallOpts{Pending: true},
		big.NewInt(int64(info.ContractID)),
	)
	if err != nil {
		log.WithError(err).Error("Failed to get random validator bitfield")
		return err
	}

	bitfield := BitfieldToString(randomBitfield)

	msg, err := beefyJustification.BuildCompleteSignatureCommitmentMessage(info, bitfield)
	if err != nil {
		return err
	}

	options := wr.makeTxOpts(ctx)

	validatorProof := beefylightclient.BeefyLightClientValidatorProof{
		Signatures:            msg.Signatures,
		Positions:             msg.ValidatorPositions,
		PublicKeys:            msg.ValidatorPublicKeys,
		PublicKeyMerkleProofs: msg.ValidatorPublicKeyMerkleProofs,
	}

	err = wr.LogBeefyFixtureDataAll(msg, info)
	if err != nil {
		log.WithError(err).Error("Failed to log complete tx input")
		return err
	}
	// Pack the input, call and unpack the results
	abi, err := beefylightclient.ContractMetaData.GetAbi()
	input, err := abi.Pack(
		"completeSignatureCommitment",
		msg.ID,
		msg.Commitment,
		validatorProof,
		msg.LatestMMRLeaf,
		msg.MMRLeafIndex,
		msg.MMRLeafCount,
		msg.MMRProofItems)
	if err != nil {
		return err
	}
	address := common.HexToAddress(wr.config.Contracts.BeefyLightClient)
	callMsg := geth.CallMsg{From: options.From, To: &address, Data: input}
	estimatedGas, err := wr.ethereumConn.GetClient().EstimateGas(ctx, callMsg)
	estimatedCost := (estimatedGas * 4000 * 50) / 1000000000
	log.WithField("estimatedGas", estimatedGas).WithField("estimatedCost", estimatedCost).WithError(err).Info("Estimated gas CompleteCommitment")
	rawCaller := beefylightclient.ContractCallerRaw{Contract: &contract.ContractCaller}
	callResult := make([]interface{}, 0)
	err = rawCaller.Call(&bind.CallOpts{Context: options.Context, From: options.From, Pending: false}, &callResult,
		"completeSignatureCommitment",
		msg.ID,
		msg.Commitment,
		validatorProof,
		msg.LatestMMRLeaf,
		msg.MMRLeafIndex,
		msg.MMRLeafCount,
		msg.MMRProofItems)
	log.WithFields(log.Fields{"error": err, "result": callResult}).Info("Test transaction")
	if err != nil {
		return err
	}

	tx, err := contract.CompleteSignatureCommitment(options,
		msg.ID,
		msg.Commitment,
		validatorProof,
		msg.LatestMMRLeaf,
		msg.MMRLeafIndex,
		msg.MMRLeafCount,
		msg.MMRProofItems)

	if err != nil {
		log.WithError(err).Error("Failed to submit transaction")
		return err
	}

	log.WithFields(log.Fields{
		"txHash": tx.Hash().Hex(),
		"gas":    tx.Gas(),
	}).Info("Complete Signature Commitment transaction submitted")

	// Update item's status in database
	log.Info("5: Updating item status from 'ReadyToComplete' to 'CompleteVerificationTxSent'")
	instructions := map[string]interface{}{
		"status":                        store.CompleteVerificationTxSent,
		"complete_verification_tx_hash": tx.Hash(),
	}

	select {
	case <-ctx.Done():
		return ctx.Err()
	case wr.databaseMessages <- store.NewDatabaseCmd(&info, store.Update, instructions):
	}

	return nil
}
