package parachain

import (
	"encoding/hex"
	"encoding/json"

	"github.com/snowfork/snowbridge/relayer/chain/relaychain"
)

// ByLeafIndex implements sort.Interface based on the LeafIndex field.
type ByParaID []relaychain.ParaHead

func (b ByParaID) Len() int           { return len(b) }
func (b ByParaID) Less(i, j int) bool { return b[i].ParaID < b[j].ParaID }
func (b ByParaID) Swap(i, j int)      { b[i], b[j] = b[j], b[i] }

type MerkleProofData struct {
	PreLeaves       PreLeaves `json:"preLeaves"`
	NumberOfLeaves  int       `json:"numberOfLeaves"`
	ProvenPreLeaf   HexBytes  `json:"provenPreLeaf"`
	ProvenLeaf      HexBytes  `json:"provenLeaf"`
	ProvenLeafIndex int64     `json:"provenLeafIndex"`
	Root            HexBytes  `json:"root"`
	Proof           Proof     `json:"proof"`
}

type PreLeaves [][]byte
type Proof [][32]byte
type HexBytes []byte

func (h HexBytes) MarshalJSON() ([]byte, error) {
	b, _ := json.Marshal("0x" + hex.EncodeToString(h))
	return b, nil
}

func (h HexBytes) String() string {
	b, _ := json.Marshal(h)
	return string(b)
}

func (h HexBytes) Hex() string {
	return "0x" + hex.EncodeToString(h)
}

func (d PreLeaves) MarshalJSON() ([]byte, error) {
	items := make([]string, 0, len(d))
	for _, v := range d {
		items = append(items, "0x"+hex.EncodeToString(v))
	}
	b, _ := json.Marshal(items)
	return b, nil
}

func (d Proof) MarshalJSON() ([]byte, error) {
	items := make([]string, 0, len(d))
	for _, v := range d {
		items = append(items, "0x"+hex.EncodeToString(v[:]))
	}
	b, _ := json.Marshal(items)
	return b, nil
}

func (d MerkleProofData) String() string {
	b, _ := json.Marshal(d)
	return string(b)
}
