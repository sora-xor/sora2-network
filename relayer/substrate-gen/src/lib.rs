#[macro_use]
extern crate codec;

/// Separated components of a secp256k1 signature.
#[derive(
    Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, scale_info::TypeInfo, Default, Debug,
)]
#[repr(C)]
pub struct SignatureParams {
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub v: u8,
}

#[subxt::subxt(
    runtime_metadata_path = "src/bytes/metadata.scale",
    generated_type_derives = "Clone, Debug"
)]
pub mod runtime {
    #[subxt(substitute_type = "eth_bridge::offchain::SignatureParams")]
    use crate::SignatureParams;
    #[subxt(substitute_type = "beefy_primitives::crypto::Public")]
    use beefy_primitives::crypto::Public;
    #[subxt(substitute_type = "bridge_types::ethashproof::DoubleNodeWithMerkleProof")]
    use bridge_types::ethashproof::DoubleNodeWithMerkleProof;
    #[subxt(substitute_type = "bridge_types::types::ChannelId")]
    use bridge_types::types::ChannelId;
    #[subxt(substitute_type = "bridge_types::types::Message")]
    use bridge_types::types::Message;
    #[subxt(substitute_type = "bridge_types::header::Header")]
    use bridge_types::Header;
    #[subxt(substitute_type = "bridge_types::header::HeaderId")]
    use bridge_types::HeaderId;
    #[subxt(substitute_type = "common::primitives::AssetId32")]
    use common::AssetId32;
    #[subxt(substitute_type = "common::primitives::LiquiditySourceType")]
    use common::LiquiditySourceType;
    #[subxt(substitute_type = "common::primitives::PredefinedAssetId")]
    use common::PredefinedAssetId;
    #[subxt(substitute_type = "common::primitives::RewardReason")]
    use common::RewardReason;
    #[subxt(substitute_type = "sp_core::ecdsa::Public")]
    use subxt::sp_core::ecdsa::Public;
    #[subxt(substitute_type = "primitive_types::H160")]
    use subxt::sp_core::H160;
    #[subxt(substitute_type = "primitive_types::H256")]
    use subxt::sp_core::H256;
    #[subxt(substitute_type = "primitive_types::H128")]
    use subxt::sp_core::H512;
    #[subxt(substitute_type = "primitive_types::U256")]
    use subxt::sp_core::U256;
}

pub use config::DefaultConfig;

pub mod config {
    use super::runtime;
    use std::fmt::Debug;
    use std::marker::PhantomData;
    use subxt::extrinsic::*;
    use subxt::sp_runtime::generic::Era;
    use subxt::sp_runtime::transaction_validity::TransactionValidityError;
    use subxt::storage::SignedExtension;
    use subxt::*;

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub struct DefaultConfig;
    impl Config for DefaultConfig {
        type Index = u32;
        type BlockNumber = u32;
        type Hash = sp_core::H256;
        type Hashing = sp_runtime::traits::BlakeTwo256;
        type AccountId = sp_runtime::AccountId32;
        type Address = Self::AccountId;
        type Header =
            sp_runtime::generic::Header<Self::BlockNumber, sp_runtime::traits::BlakeTwo256>;
        type Signature = sp_runtime::MultiSignature;
        type Extrinsic = sp_runtime::OpaqueExtrinsic;
    }

    impl ExtrinsicExtraData<DefaultConfig> for DefaultConfig {
        type AccountData = AccountData;
        type Extra = DefaultExtra<DefaultConfig>;
    }

    pub type AccountData = runtime::system::storage::Account;
    impl subxt::AccountData<DefaultConfig> for AccountData {
        fn nonce(result: &<Self as StorageEntry>::Value) -> <DefaultConfig as Config>::Index {
            result.nonce
        }
        fn storage_entry(account_id: <DefaultConfig as Config>::AccountId) -> Self {
            Self(account_id)
        }
    }

    #[derive(Encode, Decode, Clone, Eq, PartialEq, Debug, scale_info::TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct ChargeAssetTxPayment(#[codec(compact)] pub u128);

    impl SignedExtension for ChargeAssetTxPayment {
        const IDENTIFIER: &'static str = "ChargeTransactionPayment";
        type AccountId = u64;
        type Call = ();
        type AdditionalSigned = ();
        type Pre = ();
        fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
            Ok(())
        }
    }

    /// Default `SignedExtra` for substrate runtimes.
    #[derive(Encode, Decode, Clone, Eq, PartialEq, Debug, scale_info::TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct DefaultExtra<T: Config> {
        spec_version: u32,
        tx_version: u32,
        nonce: T::Index,
        genesis_hash: T::Hash,
    }

    impl<T: subxt::Config + Clone + Debug + Eq + Send + Sync> subxt::extrinsic::SignedExtra<T>
        for DefaultExtra<T>
    {
        type Extra = (
            subxt::extrinsic::CheckSpecVersion<T>,
            subxt::extrinsic::CheckTxVersion<T>,
            subxt::extrinsic::CheckGenesis<T>,
            subxt::extrinsic::CheckMortality<T>,
            subxt::extrinsic::CheckNonce<T>,
            subxt::extrinsic::CheckWeight<T>,
            ChargeAssetTxPayment,
        );
        type Parameters = ();

        fn new(
            spec_version: u32,
            tx_version: u32,
            nonce: T::Index,
            genesis_hash: T::Hash,
            _params: Self::Parameters,
        ) -> Self {
            DefaultExtra {
                spec_version,
                tx_version,
                nonce,
                genesis_hash,
            }
        }

        fn extra(&self) -> Self::Extra {
            (
                CheckSpecVersion(PhantomData, self.spec_version),
                CheckTxVersion(PhantomData, self.tx_version),
                CheckGenesis(PhantomData, self.genesis_hash),
                CheckMortality((Era::Immortal, PhantomData), self.genesis_hash),
                CheckNonce(self.nonce),
                CheckWeight(PhantomData),
                ChargeAssetTxPayment(Default::default()),
            )
        }
    }

    impl<T: Config + Clone + Debug + Eq + Send + Sync> SignedExtension for DefaultExtra<T> {
        const IDENTIFIER: &'static str = "DefaultExtra";
        type AccountId = T::AccountId;
        type Call = ();
        type AdditionalSigned =
            <<Self as SignedExtra<T>>::Extra as SignedExtension>::AdditionalSigned;
        type Pre = ();

        fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
            self.extra().additional_signed()
        }
    }
}
