package parachain

import (
	"github.com/snowfork/snowbridge/relayer/chain/relaychain"
	"github.com/vovac12/go-substrate-rpc-client/v3/types"
)

type ParaBlockWithDigest struct {
	BlockNumber         uint64
	DigestItemsWithData []DigestItemWithData
	Digest              types.Digest
}

type ParaBlockWithProofs struct {
	Block             ParaBlockWithDigest
	MMRProofResponse  types.GenerateMMRProofResponse
	MMRRootHash       types.Hash
	mmrProofLeafIndex uint64
}

type DigestItemWithData struct {
	DigestItem relaychain.AuxiliaryDigestItem
	Data       types.StorageDataRaw
}

type MessagePackage struct {
	channelID      relaychain.ChannelID
	commitmentHash types.H256
	commitmentData types.StorageDataRaw
	mmrProof       types.GenerateMMRProofResponse
	mmrRootHash    types.Hash
	digest         types.Digest
}

func CreateMessagePackages(paraBlocks []ParaBlockWithProofs, mmrLeafCount uint64) ([]MessagePackage, error) {
	var messagePackages []MessagePackage

	for _, block := range paraBlocks {
		for _, item := range block.Block.DigestItemsWithData {
			commitmentHash := item.DigestItem.AsCommitment.Hash
			commitmentData := item.Data
			messagePackage := MessagePackage{
				item.DigestItem.AsCommitment.ChannelID,
				commitmentHash,
				commitmentData,
				block.MMRProofResponse,
				block.MMRRootHash,
				block.Block.Digest,
			}
			messagePackages = append(messagePackages, messagePackage)
		}
	}

	return messagePackages, nil
}
