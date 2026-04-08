//! Setup code for [`super::command`] which would otherwise bloat that module.
//!
//! Should only be used for benchmarking as it may break in other contexts.

use crate::service::FullClient;

use codec::Encode;
use framenode_runtime as runtime;
use runtime::{AccountId, Balance};
use sc_cli::Result;
use sc_client_api::BlockBackend;
use sp_core::{sr25519, Pair};
use sp_inherents::{InherentData, InherentDataProvider};
use sp_keyring::Sr25519Keyring;
#[allow(deprecated)]
use sp_runtime::traits::transaction_extension::AsTransactionExtension;
use sp_runtime::{generic, OpaqueExtrinsic, SaturatedConversion};

use std::{sync::Arc, time::Duration};

/// Generates extrinsics for the `benchmark overhead` command.
///
/// Note: Should only be used for benchmarking.
pub struct RemarkBuilder {
    client: Arc<FullClient>,
}

impl RemarkBuilder {
    /// Creates a new [`Self`] from the given client.
    pub fn new(client: Arc<FullClient>) -> Self {
        Self { client }
    }
}

impl frame_benchmarking_cli::ExtrinsicBuilder for RemarkBuilder {
    fn pallet(&self) -> &str {
        "system"
    }

    fn extrinsic(&self) -> &str {
        "remark"
    }

    fn build(&self, nonce: u32) -> std::result::Result<OpaqueExtrinsic, &'static str> {
        let sender = Sr25519Keyring::Alice.pair();
        let extrinsic: OpaqueExtrinsic = create_benchmark_extrinsic(
            self.client.as_ref(),
            sender,
            frame_system::Call::<runtime::Runtime>::remark { remark: Vec::new() }.into(),
            nonce,
        )
        .into();

        Ok(extrinsic)
    }
}

/// Generates `Assets::transfer` extrinsics for the benchmarks.
///
/// Note: Should only be used for benchmarking.
pub struct AssetTransferBuilder {
    client: Arc<FullClient>,
    dest: AccountId,
    value: Balance,
}

impl AssetTransferBuilder {
    /// Creates a new [`Self`] from the given client.
    pub fn new(client: Arc<FullClient>, dest: AccountId, value: Balance) -> Self {
        Self {
            client,
            dest,
            value,
        }
    }
}

impl frame_benchmarking_cli::ExtrinsicBuilder for AssetTransferBuilder {
    fn pallet(&self) -> &str {
        "assets"
    }

    fn extrinsic(&self) -> &str {
        "transfer"
    }

    fn build(&self, nonce: u32) -> std::result::Result<OpaqueExtrinsic, &'static str> {
        let sender = Sr25519Keyring::Alice.pair();
        let extrinsic: OpaqueExtrinsic = create_benchmark_extrinsic(
            self.client.as_ref(),
            sender,
            assets::Call::<runtime::Runtime>::transfer {
                asset_id: runtime::GetXorAssetId::get(),
                to: self.dest.clone(),
                amount: self.value,
            }
            .into(),
            nonce,
        )
        .into();

        Ok(extrinsic)
    }
}

/// Create a transaction using the given call.
///
/// Note: Should only be used for benchmarking.
pub fn create_benchmark_extrinsic(
    client: &FullClient,
    sender: sr25519::Pair,
    call: runtime::RuntimeCall,
    nonce: u32,
) -> runtime::UncheckedExtrinsic {
    let genesis_hash = client
        .block_hash(0)
        .ok()
        .flatten()
        .expect("genesis block exists; qed");
    let best_hash = client.chain_info().best_hash;
    let best_block = client.chain_info().best_number;
    let period = runtime::BlockHashCount::get() as u64;
    #[allow(deprecated)]
    let charge_tx_payment = AsTransactionExtension(xor_fee::extension::ChargeTransactionPayment::<
        runtime::Runtime,
    >::new());
    let extra: runtime::SignedExtra = (
        frame_system::CheckSpecVersion::<runtime::Runtime>::new(),
        frame_system::CheckTxVersion::<runtime::Runtime>::new(),
        frame_system::CheckGenesis::<runtime::Runtime>::new(),
        frame_system::CheckEra::<runtime::Runtime>::from(generic::Era::mortal(
            period,
            best_block.saturated_into(),
        )),
        frame_system::CheckNonce::<runtime::Runtime>::from(nonce),
        frame_system::CheckWeight::<runtime::Runtime>::new(),
        charge_tx_payment,
    );
    let raw_payload = runtime::SignedPayload::from_raw(
        call.clone(),
        extra.clone(),
        (
            runtime::VERSION.spec_version,
            runtime::VERSION.transaction_version,
            genesis_hash,
            best_hash,
            (),
            (),
            (),
        ),
    );
    let signature = raw_payload.using_encoded(|payload| sender.sign(payload));

    runtime::UncheckedExtrinsic::new_signed(
        call,
        sp_runtime::AccountId32::from(sender.public()).into(),
        runtime::Signature::Sr25519(signature),
        extra,
    )
}

/// Generates inherent data for the `benchmark overhead` command.
pub fn inherent_benchmark_data() -> Result<InherentData> {
    let mut inherent_data = InherentData::new();
    let duration = Duration::from_millis(0);
    let timestamp = sp_timestamp::InherentDataProvider::new(duration.into());

    futures::executor::block_on(timestamp.provide_inherent_data(&mut inherent_data))
        .map_err(|error| format!("creating inherent data: {error:?}"))?;
    Ok(inherent_data)
}
