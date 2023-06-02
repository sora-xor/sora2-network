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

use sp_core::ecdsa;

use crate::cli::prelude::*;
use crate::relay::multisig_messages::RelayBuilder;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(long)]
    signer: String,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let receiver = self.sub.get_unsigned_substrate().await?;
        let sender = receiver.clone();
        let signer = ecdsa::Pair::from_string(&self.signer, None)?;
        SoraMetricsCollectorBuilder::default()
            .with_client(sender.clone())
            .with_network_id(receiver.fetch_network_id().await?)
            .build()
            .await?
            .spawn();
        SoraMetricsCollectorBuilder::default()
            .with_client(receiver.clone())
            .with_network_id(sender.fetch_network_id().await?)
            .build()
            .await?
            .spawn();
        let messages_relay = RelayBuilder::new()
            .with_sender_client(sender)
            .with_receiver_client(receiver)
            .with_signer(signer)
            .build()
            .await
            .context("build sora to sora relay")?;
        messages_relay.run().await?;
        Ok(())
    }
}
