#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod test;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod migrations;
pub mod weights;

use bridge_types::{
    traits::{
        BridgeAssetLockChecker, BridgeAssetLocker, GasTracker, MessageStatusNotifier,
        TimepointProvider,
    },
    types::{AssetKind, MessageDirection, MessageStatus},
    Address, GenericAccount, GenericNetworkId, GenericTimepoint, MainnetAccountId, H160, H256,
};
use codec::{Decode, Encode};
use common::{prelude::FixedWrapper, AssetIdOf, Balance, BalanceOf};
use common::{AssetInfoProvider, AssetManager, ReferencePriceProvider};
use frame_support::dispatch::{DispatchResult, RuntimeDebug};
use frame_support::ensure;
use frame_support::log;
use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::traits::Convert;
use sp_runtime::DispatchError;
use sp_runtime::Saturating;
use sp_std::prelude::*;

pub use weights::WeightInfo;

pub const BRIDGE_TECH_ACC_PREFIX: &[u8] = b"bridge";

#[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct BridgeRequest<AssetId> {
    source: GenericAccount,
    dest: GenericAccount,
    asset_id: AssetId,
    amount: Balance,
    status: MessageStatus,
    start_timepoint: GenericTimepoint,
    end_timepoint: GenericTimepoint,
    direction: MessageDirection,
}

#[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, TypeInfo)]
pub struct TransferLimitSettings<BlockNumber> {
    max_amount: Balance,
    period_blocks: BlockNumber,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::MainnetAccountId;
    use bridge_types::{
        substrate::ParachainAccountId,
        traits::BridgeApp,
        types::{BridgeAppInfo, BridgeAssetInfo},
    };
    use frame_support::{
        pallet_prelude::{ValueQuery, *},
        traits::{EnsureOrigin, Hooks},
        weights::Weight,
    };
    use frame_system::pallet_prelude::{BlockNumberFor, *};

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_timestamp::Config + technical::Config + permissions::Config
    {
        type RuntimeEvent: From<Event> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type EthApp: BridgeApp<Self::AccountId, H160, AssetIdOf<Self>, Balance>;

        type ERC20App: BridgeApp<Self::AccountId, H160, AssetIdOf<Self>, Balance>;

        type ParachainApp: BridgeApp<Self::AccountId, ParachainAccountId, AssetIdOf<Self>, Balance>;

        type LiberlandApp: BridgeApp<Self::AccountId, GenericAccount, AssetIdOf<Self>, Balance>;

        type HashiBridge: BridgeApp<Self::AccountId, H160, AssetIdOf<Self>, Balance>;

        type ReferencePriceProvider: ReferencePriceProvider<AssetIdOf<Self>, Balance>;

        type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        type TimepointProvider: TimepointProvider;

        type WeightInfo: WeightInfo;

        type AccountIdConverter: Convert<MainnetAccountId, Self::AccountId>;
    }

    #[pallet::storage]
    #[pallet::getter(fn transactions)]
    pub(super) type Transactions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        (GenericNetworkId, T::AccountId),
        Blake2_128Concat,
        H256,
        BridgeRequest<AssetIdOf<T>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn sender)]
    pub(super) type Senders<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        GenericNetworkId,
        Blake2_128Concat,
        H256,
        T::AccountId,
        OptionQuery,
    >;

    /// Fee paid for relayed tx on sidechain. Map ((Network ID, Address) => Cumulative Fee Paid).
    #[pallet::storage]
    #[pallet::getter(fn sidechain_fee_paid)]
    pub(super) type SidechainFeePaid<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, GenericNetworkId, Blake2_128Concat, Address, U256>;

    /// Amount of assets locked by bridge for specific network. Map ((Network ID, Asset ID) => Locked amount).
    #[pallet::storage]
    #[pallet::getter(fn locked_assets)]
    pub(super) type LockedAssets<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        GenericNetworkId,
        Blake2_128Concat,
        AssetIdOf<T>,
        Balance,
        ValueQuery,
    >;

    /// Maximum amount of assets that can be withdrawn during period of time.
    #[pallet::storage]
    #[pallet::getter(fn transfer_limit)]
    pub(super) type TransferLimit<T: Config> = StorageValue<
        _,
        TransferLimitSettings<BlockNumberFor<T>>,
        ValueQuery,
        TransferLimitDefaultValue<T>,
    >;

    #[pallet::type_value]
    pub fn TransferLimitDefaultValue<T: Config>() -> TransferLimitSettings<BlockNumberFor<T>> {
        TransferLimitSettings {
            // 50,000 USD
            max_amount: common::balance!(50000),
            // 1 hour
            period_blocks: 600u32.into(),
        }
    }

    /// Consumed transfer limit.
    #[pallet::storage]
    #[pallet::getter(fn consumed_transfer_limit)]
    pub(super) type ConsumedTransferLimit<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// Schedule for consumed transfer limit reduce.
    #[pallet::storage]
    #[pallet::getter(fn transfer_limit_unlock_schedule)]
    pub(super) type TransferLimitUnlockSchedule<T: Config> =
        StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, Balance, ValueQuery>;

    /// Assets with transfer limitation.
    #[pallet::storage]
    #[pallet::getter(fn is_asset_limited)]
    pub(super) type LimitedAssets<T: Config> =
        StorageMap<_, Blake2_128Concat, AssetIdOf<T>, bool, ValueQuery>;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let unlock_amount = TransferLimitUnlockSchedule::<T>::take(now);
            if unlock_amount > 0 {
                ConsumedTransferLimit::<T>::mutate(|v| *v = v.saturating_sub(unlock_amount));
                <T as frame_system::Config>::DbWeight::get().reads_writes(2, 2)
            } else {
                <T as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// Events for the ETH module.
    pub enum Event {
        RequestStatusUpdate(H256, MessageStatus),
        RefundFailed(H256),
    }

    #[pallet::error]
    pub enum Error<T> {
        PathIsNotAvailable,
        WrongAccountKind,
        NotEnoughLockedLiquidity,
        Overflow,
        TransferLimitReached,
        AssetAlreadyLimited,
        AssetNotLimited,
        WrongLimitSettings,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(Pallet::<T>::burn_weight())]
        pub fn burn(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            asset_id: AssetIdOf<T>,
            recipient: GenericAccount,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let sender = ensure_signed(origin)?;
            match recipient {
                GenericAccount::EVM(recipient) => {
                    if T::HashiBridge::is_asset_supported(network_id, asset_id) {
                        T::HashiBridge::transfer(network_id, asset_id, sender, recipient, amount)?;
                    } else if T::EthApp::is_asset_supported(network_id, asset_id) {
                        T::EthApp::transfer(network_id, asset_id, sender, recipient, amount)?;
                    } else if T::ERC20App::is_asset_supported(network_id, asset_id) {
                        T::ERC20App::transfer(network_id, asset_id, sender, recipient, amount)?;
                    } else {
                        frame_support::fail!(Error::<T>::PathIsNotAvailable);
                    }
                }
                GenericAccount::Parachain(recipient) => {
                    T::ParachainApp::transfer(network_id, asset_id, sender, recipient, amount)?;
                }
                GenericAccount::Sora(_) | GenericAccount::Unknown | GenericAccount::Root => {
                    frame_support::fail!(Error::<T>::WrongAccountKind);
                }
                GenericAccount::Liberland(recipient) => {
                    T::LiberlandApp::transfer(
                        network_id,
                        asset_id,
                        sender,
                        GenericAccount::Liberland(recipient),
                        amount,
                    )?;
                }
            }
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::add_limited_asset())]
        pub fn add_limited_asset(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                !Self::is_asset_limited(asset_id),
                Error::<T>::AssetAlreadyLimited
            );
            LimitedAssets::<T>::insert(asset_id, true);
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_limited_asset())]
        pub fn remove_limited_asset(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                Self::is_asset_limited(asset_id),
                Error::<T>::AssetNotLimited
            );
            LimitedAssets::<T>::remove(asset_id);
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::update_transfer_limit())]
        pub fn update_transfer_limit(
            origin: OriginFor<T>,
            settings: TransferLimitSettings<BlockNumberFor<T>>,
        ) -> DispatchResultWithPostInfo {
            T::ManagerOrigin::ensure_origin(origin)?;
            ensure!(
                settings.period_blocks > sp_runtime::traits::Zero::zero(),
                Error::<T>::WrongLimitSettings
            );
            TransferLimit::<T>::set(settings);
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn list_apps() -> Vec<BridgeAppInfo> {
            let mut res = vec![];
            res.extend(T::EthApp::list_apps());
            res.extend(T::ERC20App::list_apps());
            res.extend(T::HashiBridge::list_apps());
            res.extend(T::ParachainApp::list_apps());
            res.extend(T::LiberlandApp::list_apps());
            res
        }

        pub fn list_supported_assets(network_id: GenericNetworkId) -> Vec<BridgeAssetInfo> {
            let mut res = vec![];
            res.extend(T::EthApp::list_supported_assets(network_id));
            res.extend(T::ERC20App::list_supported_assets(network_id));
            res.extend(T::HashiBridge::list_supported_assets(network_id));
            res.extend(T::ParachainApp::list_supported_assets(network_id));
            res.extend(T::LiberlandApp::list_supported_assets(network_id));
            res
        }

        pub fn refund(
            network_id: GenericNetworkId,
            message_id: H256,
            beneficiary: GenericAccount,
            asset_id: AssetIdOf<T>,
            amount: Balance,
        ) -> DispatchResult {
            let GenericAccount::Sora(beneficiary) = beneficiary else {
                return Err(Error::<T>::WrongAccountKind.into());
            };
            let beneficiary = T::AccountIdConverter::convert(beneficiary);
            if T::HashiBridge::is_asset_supported(network_id, asset_id) {
                T::HashiBridge::refund(network_id, message_id, beneficiary, asset_id, amount)?;
            } else if T::ParachainApp::is_asset_supported(network_id, asset_id) {
                T::ParachainApp::refund(network_id, message_id, beneficiary, asset_id, amount)?;
            } else if T::LiberlandApp::is_asset_supported(network_id, asset_id) {
                T::LiberlandApp::refund(network_id, message_id, beneficiary, asset_id, amount)?;
            } else if T::EthApp::is_asset_supported(network_id, asset_id) {
                T::EthApp::refund(network_id, message_id, beneficiary, asset_id, amount)?;
            } else {
                T::ERC20App::refund(network_id, message_id, beneficiary, asset_id, amount)?;
            }
            Ok(())
        }

        /// Returns the maximum weight which can be consumed by burn call.
        fn burn_weight() -> Weight {
            T::HashiBridge::transfer_weight()
                .max(T::EthApp::transfer_weight())
                .max(T::ERC20App::transfer_weight())
                .max(T::ParachainApp::transfer_weight())
                .max(T::LiberlandApp::transfer_weight())
                .saturating_add(T::HashiBridge::is_asset_supported_weight())
                .saturating_add(T::EthApp::is_asset_supported_weight())
                .saturating_add(T::ERC20App::is_asset_supported_weight())
        }
    }
}

impl<T: Config> GasTracker<Balance> for Pallet<T>
where
    T::AccountId: From<bridge_types::MainnetAccountId>,
{
    /// Records fee paid by relayer for message submission.
    /// - network_id - ethereum network id,
    /// - batch_nonce - batch nonce,
    /// - ethereum_relayer_address - relayer that had paid for the batch submission,
    /// - gas_used - gas paid for batch relaying,
    /// - gas_price - ethereum base fee in the block when batch was submitted.
    fn record_tx_fee(
        network_id: GenericNetworkId,
        batch_nonce: u64,
        ethereum_relayer_address: Address,
        gas_used: U256,
        gas_price: U256,
    ) {
        log::debug!(
            "Record tx fee: batch_nonce={}, ethereum_relayer_address={}, gas_used={}, gas_price={}",
            batch_nonce,
            ethereum_relayer_address,
            gas_used,
            gas_price,
        );

        let tx_fee = gas_used * gas_price;

        SidechainFeePaid::<T>::mutate(
            network_id,
            ethereum_relayer_address,
            |maybe_cumulative_fee| {
                let cumulative_fee = maybe_cumulative_fee.get_or_insert(U256::from(0));
                *cumulative_fee += tx_fee;
            },
        );
    }
}

impl<T: Config> MessageStatusNotifier<AssetIdOf<T>, T::AccountId, Balance> for Pallet<T>
where
    MainnetAccountId: From<T::AccountId>,
{
    fn update_status(
        network_id: GenericNetworkId,
        message_id: H256,
        mut new_status: MessageStatus,
        end_timepoint: GenericTimepoint,
    ) {
        let sender = match Senders::<T>::get(network_id, message_id) {
            Some(sender) => sender,
            None => {
                log::warn!(
                    "Message status update called for unknown message: {:?} {:?}",
                    network_id,
                    message_id
                );
                return;
            }
        };
        Transactions::<T>::mutate((network_id, sender), message_id, |req| {
            if let Some(req) = req {
                if new_status == MessageStatus::Failed
                    && req.direction == MessageDirection::Outbound
                {
                    match Pallet::<T>::refund(
                        network_id,
                        message_id,
                        req.source.clone(),
                        req.asset_id,
                        req.amount,
                    ) {
                        Ok(_) => {
                            new_status = MessageStatus::Refunded;
                        }
                        Err(_) => {
                            Self::deposit_event(Event::RefundFailed(message_id));
                        }
                    }
                }
                req.status = new_status;
                req.end_timepoint = end_timepoint;

                Self::deposit_event(Event::RequestStatusUpdate(message_id, new_status));
            }
        })
    }

    fn inbound_request(
        network_id: GenericNetworkId,
        message_id: H256,
        source: GenericAccount,
        dest: T::AccountId,
        asset_id: AssetIdOf<T>,
        amount: Balance,
        start_timepoint: GenericTimepoint,
        status: MessageStatus,
    ) {
        Self::deposit_event(Event::RequestStatusUpdate(message_id, status));
        Senders::<T>::insert(&network_id, &message_id, &dest);
        Transactions::<T>::insert(
            (&network_id, &dest),
            &message_id,
            BridgeRequest {
                source,
                dest: GenericAccount::Sora(dest.clone().into()),
                asset_id,
                amount,
                status,
                start_timepoint,
                end_timepoint: T::TimepointProvider::get_timepoint(),
                direction: MessageDirection::Inbound,
            },
        );
    }

    fn outbound_request(
        network_id: GenericNetworkId,
        message_id: H256,
        source: T::AccountId,
        dest: GenericAccount,
        asset_id: AssetIdOf<T>,
        amount: Balance,
        status: MessageStatus,
    ) {
        Self::deposit_event(Event::RequestStatusUpdate(message_id, status));
        Senders::<T>::insert(&network_id, &message_id, &source);
        Transactions::<T>::insert(
            (&network_id, &source),
            &message_id,
            BridgeRequest {
                source: GenericAccount::Sora(source.clone().into()),
                dest,
                asset_id,
                amount,
                status,
                start_timepoint: T::TimepointProvider::get_timepoint(),
                end_timepoint: GenericTimepoint::Pending,
                direction: MessageDirection::Outbound,
            },
        );
    }
}

impl<T: Config> Pallet<T> {
    pub fn bridge_tech_account(
        network_id: GenericNetworkId,
    ) -> <T as technical::Config>::TechAccountId {
        common::FromGenericPair::from_generic_pair(
            BRIDGE_TECH_ACC_PREFIX.to_vec(),
            network_id.encode(),
        )
    }

    pub fn bridge_account(network_id: GenericNetworkId) -> Result<T::AccountId, DispatchError> {
        technical::Pallet::<T>::tech_account_id_to_account_id(&Self::bridge_tech_account(
            network_id,
        ))
    }
}

impl<T: Config> BridgeAssetLocker<T::AccountId> for Pallet<T> {
    type AssetId = AssetIdOf<T>;
    type Balance = Balance;

    fn lock_asset(
        network_id: GenericNetworkId,
        asset_kind: bridge_types::types::AssetKind,
        who: &T::AccountId,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> DispatchResult {
        Self::before_asset_lock(network_id, asset_kind, asset_id, amount)?;
        match asset_kind {
            bridge_types::types::AssetKind::Thischain => {
                let bridge_account = Self::bridge_tech_account(network_id);
                technical::Pallet::<T>::transfer_in(asset_id, who, &bridge_account, *amount)?;
            }
            bridge_types::types::AssetKind::Sidechain => {
                let bridge_account = Self::bridge_account(network_id)?;
                technical::Pallet::<T>::ensure_account_registered(&bridge_account)?;
                T::AssetManager::burn_from(*asset_id, &bridge_account, who, *amount)?;
            }
        }
        Ok(())
    }

    fn unlock_asset(
        network_id: GenericNetworkId,
        asset_kind: bridge_types::types::AssetKind,
        who: &T::AccountId,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> DispatchResult {
        Self::before_asset_unlock(network_id, asset_kind, asset_id, amount)?;
        match asset_kind {
            bridge_types::types::AssetKind::Thischain => {
                let bridge_account = Self::bridge_tech_account(network_id);
                technical::Pallet::<T>::transfer_out(asset_id, &bridge_account, who, *amount)?;
            }
            bridge_types::types::AssetKind::Sidechain => {
                let bridge_account = Self::bridge_account(network_id)?;
                technical::Pallet::<T>::ensure_account_registered(&bridge_account)?;
                T::AssetManager::mint_to(*asset_id, &bridge_account, who, *amount)?;
            }
        }
        Ok(())
    }
}

impl<T: Config> BridgeAssetLockChecker<AssetIdOf<T>, Balance> for Pallet<T> {
    fn before_asset_lock(
        network_id: GenericNetworkId,
        asset_kind: bridge_types::types::AssetKind,
        asset_id: &AssetIdOf<T>,
        amount: &Balance,
    ) -> DispatchResult {
        LockedAssets::<T>::try_mutate::<_, _, (), DispatchError, _>(
            network_id,
            asset_id,
            |locked_amount| match asset_kind {
                AssetKind::Thischain => {
                    *locked_amount = locked_amount
                        .checked_add(*amount)
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(())
                }
                AssetKind::Sidechain => {
                    *locked_amount = locked_amount
                        .checked_sub(*amount)
                        .ok_or(Error::<T>::NotEnoughLockedLiquidity)?;
                    Ok(())
                }
            },
        )?;
        if Self::is_asset_limited(&asset_id) {
            if let Ok(reference_price) = T::ReferencePriceProvider::get_reference_price(asset_id) {
                let reference_amount =
                    FixedWrapper::from(reference_price) * FixedWrapper::from(*amount);
                let reference_amount = reference_amount
                    .try_into_balance()
                    .map_err(|_| Error::<T>::Overflow)?;
                let transfer_limit = TransferLimit::<T>::get();
                ConsumedTransferLimit::<T>::try_mutate(|value| {
                    *value = value
                        .checked_add(reference_amount)
                        .ok_or(Error::<T>::Overflow)?;
                    ensure!(
                        *value < transfer_limit.max_amount,
                        Error::<T>::TransferLimitReached
                    );
                    DispatchResult::Ok(())
                })?;
                TransferLimitUnlockSchedule::<T>::try_mutate(
                    frame_system::Pallet::<T>::block_number()
                        .saturating_add(transfer_limit.period_blocks),
                    |value| {
                        *value = value
                            .checked_add(reference_amount)
                            .ok_or(Error::<T>::Overflow)?;
                        DispatchResult::Ok(())
                    },
                )?;
            }
        }
        Ok(())
    }

    fn before_asset_unlock(
        network_id: GenericNetworkId,
        asset_kind: bridge_types::types::AssetKind,
        asset_id: &AssetIdOf<T>,
        amount: &Balance,
    ) -> DispatchResult {
        LockedAssets::<T>::try_mutate::<_, _, (), DispatchError, _>(
            network_id,
            asset_id,
            |locked_amount| match asset_kind {
                AssetKind::Thischain => {
                    *locked_amount = locked_amount
                        .checked_sub(*amount)
                        .ok_or(Error::<T>::NotEnoughLockedLiquidity)?;
                    Ok(())
                }
                AssetKind::Sidechain => {
                    *locked_amount = locked_amount
                        .checked_add(*amount)
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(())
                }
            },
        )?;
        Ok(())
    }
}

impl<T: Config> bridge_types::traits::BridgeAssetRegistry<T::AccountId, AssetIdOf<T>>
    for Pallet<T>
{
    type AssetName = common::AssetName;
    type AssetSymbol = common::AssetSymbol;

    fn register_asset(
        network_id: GenericNetworkId,
        name: Self::AssetName,
        symbol: Self::AssetSymbol,
    ) -> Result<AssetIdOf<T>, DispatchError> {
        technical::Pallet::<T>::register_tech_account_id_if_not_exist(&Self::bridge_tech_account(
            network_id,
        ))?;
        let owner = Self::bridge_account(network_id)?;
        let asset_id =
            T::AssetManager::register_from(&owner, symbol, name, 18, 0, true, None, None)?;
        Ok(asset_id)
    }

    fn manage_asset(network_id: GenericNetworkId, asset_id: AssetIdOf<T>) -> DispatchResult {
        technical::Pallet::<T>::register_tech_account_id_if_not_exist(&Self::bridge_tech_account(
            network_id,
        ))?;
        let manager = Self::bridge_account(network_id)?;
        let scope = permissions::Scope::Limited(common::hash(&asset_id));
        for permission_id in [permissions::BURN, permissions::MINT] {
            if permissions::Pallet::<T>::check_permission_with_scope(
                manager.clone(),
                permission_id,
                &scope,
            )
            .is_err()
            {
                permissions::Pallet::<T>::assign_permission(
                    manager.clone(),
                    &manager,
                    permission_id,
                    scope,
                )?;
            }
        }
        Ok(())
    }

    fn get_raw_info(asset_id: AssetIdOf<T>) -> bridge_types::types::RawAssetInfo {
        let (asset_symbol, asset_name, precision, ..) =
            <T as technical::Config>::AssetInfoProvider::get_asset_info(&asset_id);
        bridge_types::types::RawAssetInfo {
            name: asset_name.0,
            symbol: asset_symbol.0,
            precision,
        }
    }

    fn ensure_asset_exists(asset_id: AssetIdOf<T>) -> bool {
        <T as technical::Config>::AssetInfoProvider::asset_exists(&asset_id)
    }
}
