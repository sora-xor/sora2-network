// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::prelude::*;
use bridge_types::types::LeafExtraData;
use bridge_types::H256;
use codec::IoReader;
use common::{AssetId32, PredefinedAssetId};
pub use parachain_gen::{parachain_runtime, SoraExtrinsicParams as ParachainExtrinsicParams};
use sp_core::Bytes;
use sp_mmr_primitives::Proof;
pub use substrate_gen::{
    runtime as mainnet_runtime, SoraExtrinsicParams as MainnetExtrinsicParams,
};
pub use subxt::rpc::ChainBlock;
pub use subxt::rpc::Subscription;
use subxt::tx::TxPayload;
use subxt::Config as SubxtConfig;
use subxt::OnlineClient;

pub type ApiInner<T> = OnlineClient<<T as ConfigExt>::Config>;
pub type PairSigner<T> = <T as ConfigExt>::Signer;
pub type AccountId<T> = <<T as ConfigExt>::Config as SubxtConfig>::AccountId;
pub type Address<T> = <<T as ConfigExt>::Config as SubxtConfig>::Address;
pub type Index<T> = <<T as ConfigExt>::Config as SubxtConfig>::Index;
pub type SubxtBlockHash<T> = <<T as ConfigExt>::Config as SubxtConfig>::Hash;
pub type BlockNumber<T> = <T as ConfigExt>::BlockNumber;
pub type BlockHash<T> = <T as ConfigExt>::Hash;
pub type Signature<T> = <<T as ConfigExt>::Config as SubxtConfig>::Signature;
pub type Header<T> = <<T as ConfigExt>::Config as SubxtConfig>::Header;
pub type ExtrinsicParams<T> = <<T as ConfigExt>::Config as SubxtConfig>::ExtrinsicParams;
pub type OtherParams<T> =
    <ExtrinsicParams<T> as subxt::tx::ExtrinsicParams<Index<T>, SubxtBlockHash<T>>>::OtherParams;
pub type MmrHash = H256;
pub type LeafExtra = LeafExtraData<H256, H256>;
pub type BeefySignedCommitment<T> =
    sp_beefy::VersionedFinalityProof<BlockNumber<T>, sp_beefy::crypto::Signature>;
pub type BeefyCommitment<T> = sp_beefy::Commitment<BlockNumber<T>>;
pub type MmrLeaf<T> = sp_beefy::mmr::MmrLeaf<BlockNumber<T>, BlockHash<T>, MmrHash, LeafExtra>;
pub type AssetId = AssetId32<PredefinedAssetId>;
pub type MaxU32 = sp_runtime::traits::ConstU32<{ core::u32::MAX }>;
pub type UnboundedGenericCommitment = bridge_types::GenericCommitment<MaxU32, MaxU32>;
pub type GenericCommitmentWithBlockOf<T> =
    bridge_types::types::GenericCommitmentWithBlock<BlockNumber<T>, MaxU32, MaxU32>;

#[derive(Debug, Clone)]
pub struct LeafProof<T: ConfigExt> {
    pub block_hash: BlockHash<T>,
    pub leaf: MmrLeaf<T>,
    pub proof: Proof<MmrHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedBeefyCommitment(pub Bytes);

impl EncodedBeefyCommitment {
    pub fn decode<T: ConfigExt>(&self) -> AnyResult<BeefySignedCommitment<T>> {
        let mut reader = IoReader(&self.0[..]);
        Ok(Decode::decode(&mut reader)?)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BlockNumberOrHash {
    Number(u64),
    Hash(H256),
    Best,
    Finalized,
}

impl From<()> for BlockNumberOrHash {
    fn from(_: ()) -> Self {
        BlockNumberOrHash::Best
    }
}

impl From<u64> for BlockNumberOrHash {
    fn from(number: u64) -> Self {
        BlockNumberOrHash::Number(number)
    }
}

impl From<u32> for BlockNumberOrHash {
    fn from(number: u32) -> Self {
        BlockNumberOrHash::Number(number.into())
    }
}

impl From<H256> for BlockNumberOrHash {
    fn from(hash: H256) -> Self {
        BlockNumberOrHash::Hash(hash)
    }
}

pub struct UnvalidatedTxPayload<'a, P: TxPayload>(pub &'a P);

impl<'a, P: TxPayload> TxPayload for UnvalidatedTxPayload<'a, P> {
    fn encode_call_data_to(
        &self,
        metadata: &subxt::Metadata,
        out: &mut Vec<u8>,
    ) -> Result<(), subxt::Error> {
        self.0.encode_call_data_to(metadata, out)
    }
}
