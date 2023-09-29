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

use std::collections::BTreeSet;

use crate::prelude::*;
use crate::relay::messages_subscription::load_digest;
use crate::substrate::OtherParams;
use bridge_types::types::AuxiliaryDigest;
use bridge_types::{GenericNetworkId, SubNetworkId, H256};
use sp_core::ecdsa;
use sp_runtime::traits::Keccak256;

pub struct RelayBuilder<S: SenderConfig, R: ReceiverConfig> {
    sender: Option<SubUnsignedClient<S>>,
    receiver: Option<SubUnsignedClient<R>>,
    signer: Option<ecdsa::Pair>,
}

impl<S: SenderConfig, R: ReceiverConfig> Default for RelayBuilder<S, R> {
    fn default() -> Self {
        Self {
            sender: None,
            receiver: None,
            signer: None,
        }
    }
}

impl<S, R> RelayBuilder<S, R>
where
    S: SenderConfig,
    R: ReceiverConfig,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_sender_client(mut self, sender: SubUnsignedClient<S>) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn with_receiver_client(mut self, receiver: SubUnsignedClient<R>) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_signer(mut self, signer: ecdsa::Pair) -> Self {
        self.signer = Some(signer);
        self
    }

    pub async fn build(self) -> AnyResult<Relay<S, R>> {
        let sender = self.sender.expect("sender client is needed");
        let receiver = self.receiver.expect("receiver client is needed");
        let signer = self.signer.expect("signer is needed");
        let sender_network_id = sender.constant_fetch_or_default(&S::network_id())?;

        let GenericNetworkId::Sub(sender_network_id) = sender_network_id else {
            return Err(anyhow::anyhow!("Error! Sender is NOT a Substrate Network!"));
        };

        let receiver_network_id = receiver.constant_fetch_or_default(&R::network_id())?;

        let GenericNetworkId::Sub(receiver_network_id) = receiver_network_id else {
            return Err(anyhow::anyhow!("Error! Reciever is NOT a Substrate Network!"));
        };

        Ok(Relay {
            sender,
            receiver,
            signer,
            receiver_network_id,
            sender_network_id,
        })
    }
}

#[derive(Clone)]
pub struct Relay<S: SenderConfig, R: ReceiverConfig> {
    sender: SubUnsignedClient<S>,
    receiver: SubUnsignedClient<R>,
    signer: ecdsa::Pair,
    receiver_network_id: SubNetworkId,
    sender_network_id: SubNetworkId,
}

impl<S, R> Relay<S, R>
where
    S: SenderConfig,
    R: ReceiverConfig,
    OtherParams<R>: Default,
    OtherParams<S>: Default,
{
    async fn inbound_channel_nonce(&self) -> AnyResult<u64> {
        let storage = R::substrate_bridge_inbound_nonce(self.sender_network_id);
        let nonce = self.receiver.storage_fetch_or_default(&storage, ()).await?;
        Ok(nonce)
    }

    async fn outbound_channel_nonce(&self) -> AnyResult<u64> {
        let nonce = self
            .sender
            .storage_fetch_or_default(
                &S::bridge_outbound_nonce(self.receiver_network_id.into()),
                (),
            )
            .await?;
        Ok(nonce)
    }

    async fn approvals(&self, message: H256) -> AnyResult<Vec<ecdsa::Signature>> {
        let peers = self.receiver_peers().await?;
        let approvals = self
            .sender
            .storage_fetch_or_default(&S::approvals(self.receiver_network_id.into(), message), ())
            .await?;
        let mut acceptable_approvals = vec![];
        for approval in approvals {
            let public = approval
                .1
                .recover_prehashed(&message.0)
                .ok_or(anyhow!("Wrong signature in data signer pallet"))?;
            if peers.contains(&public) {
                acceptable_approvals.push(approval.1);
            }
        }
        Ok(acceptable_approvals)
    }

    async fn sender_peers(&self) -> AnyResult<BTreeSet<ecdsa::Public>> {
        let peers = self
            .sender
            .storage_fetch(&S::peers(self.receiver_network_id.into()), ())
            .await?
            .unwrap_or_default();
        Ok(peers)
    }

    async fn receiver_peers(&self) -> AnyResult<BTreeSet<ecdsa::Public>> {
        let peers = self
            .receiver
            .storage_fetch(&R::peers(self.sender_network_id.into()), ())
            .await?
            .unwrap_or_default()
            .into_iter()
            .collect();
        Ok(peers)
    }

    pub async fn run(self) -> AnyResult<()> {
        loop {
            let public = self.signer.public();
            let peers = self.sender_peers().await?;
            if !peers.contains(&public) {
                info!("Peer is not in trusted list, waiting...");
                tokio::time::sleep(S::average_block_time()).await;
            } else {
                break;
            }
        }
        let mut interval = tokio::time::interval(S::average_block_time());
        loop {
            interval.tick().await;
            let inbound_nonce = self.inbound_channel_nonce().await?;
            let outbound_nonce = self.outbound_channel_nonce().await?;
            if inbound_nonce >= outbound_nonce {
                if inbound_nonce > outbound_nonce {
                    error!(
                        "Inbound channel nonce is higher than outbound channel nonce: {} > {}",
                        inbound_nonce, outbound_nonce
                    );
                }
                continue;
            }
            for nonce in (inbound_nonce + 1)..=outbound_nonce {
                let offchain_data = self
                    .sender
                    .bridge_commitment(self.receiver_network_id.into(), nonce)
                    .await?;
                let commitment_hash = offchain_data.commitment.hash();
                let digest: AuxiliaryDigest = load_digest(
                    &self.sender,
                    self.receiver_network_id.into(),
                    offchain_data.block_number,
                    commitment_hash,
                )
                .await?;
                let digest_hash = Keccak256::hash_of(&digest);
                trace!("Digest hash: {}", digest_hash);
                let peers = self.receiver_peers().await?;
                let approvals = self.approvals(digest_hash).await?;
                if (approvals.len() as u32) < bridge_types::utils::threshold(peers.len() as u32) {
                    let signature = self.signer.sign_prehashed(&digest_hash.0);
                    let call = S::submit_signature(
                        self.receiver_network_id.into(),
                        digest_hash,
                        signature,
                    );
                    self.sender.submit_unsigned_extrinsic(&call).await?;
                }
                let approvals = self.approvals(digest_hash).await?;
                if (approvals.len() as u32) < bridge_types::utils::threshold(peers.len() as u32) {
                    info!(
                    "Still not enough signatures, probably another relayer will submit commitment"
                );
                    continue;
                }
                let call = R::submit_messages_commitment(
                    self.sender_network_id.into(),
                    offchain_data.commitment,
                    R::multisig_proof(digest, approvals),
                );
                if let Err(err) = self.receiver.submit_unsigned_extrinsic(&call).await {
                    error!("Failed to submit messages, probably another relayer already submitted it: {:?}", err);
                }
            }
        }
    }
}
