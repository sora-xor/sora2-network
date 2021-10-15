package parachain

import (
	"github.com/snowfork/snowbridge/relayer/chain/relaychain"
	"github.com/vovac12/go-substrate-rpc-client/v3/types"
)

type ParaBlockWithDigest struct {
	BlockNumber         uint64
	DigestItemsWithData []DigestItemWithData
}

type ParaBlockWithProofs struct {
	Block             ParaBlockWithDigest
	MMRProofResponse  types.GenerateMMRProofResponse
	MMRRootHash       types.Hash
	Header            types.Header
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
	paraHead       types.Header
	mmrProof       types.GenerateMMRProofResponse
	mmrRootHash    types.Hash
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
				block.Header,
				block.MMRProofResponse,
				block.MMRRootHash,
			}
			messagePackages = append(messagePackages, messagePackage)
		}
	}

	return messagePackages, nil
}
