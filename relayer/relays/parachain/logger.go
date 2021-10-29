package parachain

import (
	"encoding/hex"
	"encoding/json"
	"math/big"

	"github.com/ethereum/go-ethereum/common"
	log "github.com/sirupsen/logrus"
	"github.com/snowfork/snowbridge/relayer/contracts/basic"
	"github.com/snowfork/snowbridge/relayer/contracts/incentivized"
	"github.com/snowfork/snowbridge/relayer/crypto/keccak"
	"github.com/snowfork/snowbridge/relayer/crypto/merkle"
	"github.com/vovac12/go-substrate-rpc-client/v3/types"
)

type ParaVerifyInputLog struct {
	OwnParachainHeadPrefixBytes string           `json:"ownParachainHeadPrefixBytes"`
	OwnParachainHeadSuffixBytes string           `json:"ownParachainHeadSuffixBytes"`
	ParachainHeadProof          ParaHeadProofLog `json:"parachainHeadProof"`
}

type ParaHeadProofLog struct {
	Pos   *big.Int `json:"pos"`
	Width *big.Int `json:"width"`
	Proof []string `json:"proof"`
}

type BeefyMMRLeafPartialLog struct {
	Version              uint8  `json:"version"`
	ParentNumber         uint32 `json:"parentNumber"`
	ParentHash           string `json:"parentHash"`
	NextAuthoritySetId   uint64 `json:"nextAuthoritySetId"` // revive:disable-line
	NextAuthoritySetLen  uint32 `json:"nextAuthoritySetLen"`
	NextAuthoritySetRoot string `json:"nextAuthoritySetRoot"`
}

type BasicInboundChannelMessageLog struct {
	Target  common.Address `json:"target"`
	Nonce   uint64         `json:"nonce"`
	Payload string         `json:"payload"`
}

type IncentivizedInboundChannelMessageLog struct {
	Target  common.Address `json:"target"`
	Nonce   uint64         `json:"nonce"`
	Fee     *big.Int       `json:"fee"`
	Payload string         `json:"payload"`
}

type BasicSubmitInput struct {
	Messages            []BasicInboundChannelMessageLog `json:"_messages"`
	ParaVerifyInput     ParaVerifyInputLog              `json:"_paraVerifyInput"`
	BeefyMMRLeafPartial BeefyMMRLeafPartialLog          `json:"_beefyMMRLeafPartial"`
	BeefyMMRLeafIndex   int64                           `json:"_beefyMMRLeafIndex"`
	BeefyLeafCount      int64                           `json:"_beefyLeafCount"`
	BeefyMMRProof       []string                        `json:"_beefyMMRProof"`
}

type IncentivizedSubmitInput struct {
	Messages            []IncentivizedInboundChannelMessageLog `json:"_messages"`
	ParaVerifyInput     ParaVerifyInputLog                     `json:"_paraVerifyInput"`
	BeefyMMRLeafPartial BeefyMMRLeafPartialLog                 `json:"_beefyMMRLeafPartial"`
	BeefyMMRLeafIndex   int64                                  `json:"_beefyMMRLeafIndex"`
	BeefyLeafCount      int64                                  `json:"_beefyLeafCount"`
	BeefyMMRProof       []string                               `json:"_beefyMMRProof"`
}

func (wr *EthereumChannelWriter) logBasicTx(
	messages []basic.BasicInboundChannelMessage,
	proof merkle.SimplifiedMMRProof,
	mmrLeaf types.MMRLeaf,
	commitmentHash types.H256, mmrRootHash types.Hash,
) error {

	var basicMessagesLog []BasicInboundChannelMessageLog
	for _, item := range messages {
		basicMessagesLog = append(basicMessagesLog, BasicInboundChannelMessageLog{
			Target:  item.Target,
			Nonce:   item.Nonce,
			Payload: "0x" + hex.EncodeToString(item.Payload),
		})
	}
	var beefyMMRProofString []string
	for _, item := range proof.MerkleProofItems {
		beefyMMRProofString = append(beefyMMRProofString, "0x"+hex.EncodeToString(item[:]))
	}
	input := &BasicSubmitInput{
		Messages:      basicMessagesLog,
		BeefyMMRProof: beefyMMRProofString,
	}
	b, err := json.Marshal(input)
	if err != nil {
		return err
	}

	mmrLeafEncoded, _ := types.EncodeToBytes(mmrLeaf)
	mmrLeafOpaqueEncoded, _ := types.EncodeToHexString(mmrLeafEncoded)
	mmrLeafOpaqueEncodedBytes, _ := types.EncodeToBytes(mmrLeafEncoded)

	log.WithFields(log.Fields{
		"input":                       string(b),
		"commitmentHash":              "0x" + hex.EncodeToString(commitmentHash[:]),
		"paraHeadProofRootMerkleLeaf": "0x" + hex.EncodeToString(mmrLeaf.ParachainHeads[:]),
		"Leaf.Digest":                 mmrLeaf.DigestHash.Hex(),
		"mmrLeafOpaqueEncoded":        mmrLeafOpaqueEncoded,
		"mmrRootHash":                 "0x" + hex.EncodeToString(mmrRootHash[:]),
	}).Info("Submitting tx")

	hasher := &keccak.Keccak256{}

	log.WithFields(log.Fields{
		"mmrLeafOpaqueEncoded": mmrLeafOpaqueEncoded,
		"hashedOpaqueLeaf":     "0x" + hex.EncodeToString(hasher.Hash(mmrLeafOpaqueEncodedBytes)),
		"hashedLeaf":           "0x" + hex.EncodeToString(hasher.Hash(mmrLeafEncoded)),
	}).Info("DAT LEAF")

	return nil
}

func (wr *EthereumChannelWriter) logIncentivizedTx(
	messages []incentivized.IncentivizedInboundChannelMessage,
	proof merkle.SimplifiedMMRProof,
	mmrLeaf types.MMRLeaf,
	commitmentHash types.H256, mmrRootHash types.Hash,
) error {
	var incentivizedMessagesLog []IncentivizedInboundChannelMessageLog
	for _, item := range messages {
		incentivizedMessagesLog = append(incentivizedMessagesLog, IncentivizedInboundChannelMessageLog{
			Target:  item.Target,
			Nonce:   item.Nonce,
			Fee:     item.Fee,
			Payload: "0x" + hex.EncodeToString(item.Payload),
		})
	}

	var beefyMMRProofString []string
	for _, item := range proof.MerkleProofItems {
		beefyMMRProofString = append(beefyMMRProofString, "0x"+hex.EncodeToString(item[:]))
	}
	input := &IncentivizedSubmitInput{
		Messages:      incentivizedMessagesLog,
		BeefyMMRProof: beefyMMRProofString,
	}
	b, err := json.Marshal(input)
	if err != nil {
		return err
	}

	mmrLeafEncoded, _ := types.EncodeToBytes(mmrLeaf)
	mmrLeafOpaqueEncoded, _ := types.EncodeToHexString(mmrLeafEncoded)
	mmrLeafOpaqueEncodedBytes, _ := types.EncodeToBytes(mmrLeafEncoded)

	log.WithFields(log.Fields{
		"input":                       string(b),
		"commitmentHash":              "0x" + hex.EncodeToString(commitmentHash[:]),
		"paraHeadProofRootMerkleLeaf": "0x" + hex.EncodeToString(mmrLeaf.ParachainHeads[:]),
		"Leaf.Digest":                 mmrLeaf.DigestHash.Hex(),
		"mmrLeafOpaqueEncoded":        mmrLeafOpaqueEncoded,
		"mmrRootHash":                 "0x" + hex.EncodeToString(mmrRootHash[:]),
	}).Info("Submitting tx")

	hasher := &keccak.Keccak256{}

	log.WithFields(log.Fields{
		"mmrLeafOpaqueEncoded": mmrLeafOpaqueEncoded,
		"hashedOpaqueLeaf":     "0x" + hex.EncodeToString(hasher.Hash(mmrLeafOpaqueEncodedBytes)),
		"hashedLeaf":           "0x" + hex.EncodeToString(hasher.Hash(mmrLeafEncoded)),
	}).Info("DAT LEAF")
	return nil
}
