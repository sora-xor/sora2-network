use super::*;
use crate::prelude::*;
use clap::*;

#[derive(Args, Clone, Debug)]
pub(super) struct TransferToEthereum {
    #[clap(flatten)]
    url: SubstrateUrl,
    #[clap(flatten)]
    key: SubstrateKey,
}

impl TransferToEthereum {
    pub(super) async fn run(&self) -> AnyResult<()> {
        Ok(())
    }
}
