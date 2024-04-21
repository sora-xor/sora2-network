use common::{generate_storage_instance, AssetIdOf, AssetInfoProvider};
use frame_support::dispatch::Weight;
use frame_support::pallet_prelude::{StorageValue, ValueQuery};
use frame_support::traits::Get;
use orml_tokens::{AccountData, Accounts};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

use crate::{Config, PoolProviders, Properties, TotalIssuances};

generate_storage_instance!(PoolXYK, MarkerTokensIndex);
type OldMarkerTokensIndex<AssetId> =
    StorageValue<MarkerTokensIndexOldInstance, BTreeSet<AssetId>, ValueQuery>;

pub fn migrate<T: Config>() -> Weight {
    if OldMarkerTokensIndex::<AssetIdOf<T>>::exists() {
        OldMarkerTokensIndex::<AssetIdOf<T>>::kill();
    }
    let mut acc_asset_currs = Vec::new();
    Properties::<T>::translate::<(T::AccountId, T::AccountId, AssetIdOf<T>), _>(
        |_ba, _ta, (reserves_acc, fee_acc, marker_asset)| {
            let currency: <T as orml_tokens::Config>::CurrencyId = marker_asset.clone().into();
            acc_asset_currs.push((reserves_acc.clone(), marker_asset, currency));
            Some((reserves_acc, fee_acc))
        },
    );

    for (reserves_acc, asset, _) in &acc_asset_currs {
        let total_issuance =
            if let Ok(issuance) = <T as Config>::AssetInfoProvider::total_issuance(asset) {
                issuance
            } else {
                continue;
            };
        TotalIssuances::<T>::insert(reserves_acc, total_issuance);
    }

    Accounts::<T>::translate(
        |account, currency, data: AccountData<<T as orml_tokens::Config>::Balance>| {
            if let Some((pool_acc, _, _)) = acc_asset_currs
                .iter()
                .find(|(_, _, probe_currency)| probe_currency == &currency)
            {
                let balance: u128 = data.free.unique_saturated_into();
                PoolProviders::<T>::insert(&pool_acc, account, balance);
                None
            } else {
                Some(data)
            }
        },
    );

    T::BlockWeights::get().max_block
}

#[cfg(test)]
mod tests {
    use common::{
        balance, generate_storage_instance, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION,
    };
    use frame_support::pallet_prelude::StorageDoubleMap;
    use frame_support::Blake2_128Concat;
    use hex_literal::hex;
    use sp_std::collections::btree_set::BTreeSet;

    use crate::mock::{AccountId, AssetId, ExtBuilder, Runtime, ALICE, BOB};
    use crate::{PoolProviders, Properties, TotalIssuances};

    use super::OldMarkerTokensIndex;

    generate_storage_instance!(PoolXYK, Properties);

    type OldProperties<AccountId, AssetId> = StorageDoubleMap<
        PropertiesOldInstance,
        Blake2_128Concat,
        AssetId,
        Blake2_128Concat,
        AssetId,
        (AccountId, AccountId, AssetId),
    >;

    #[test]
    fn test() {
        ExtBuilder::default().build().execute_with(|| {
            let asset1 = AssetId::from_bytes(
                hex!("0200000700000000000000000000000000000000000000000000000000000000").into(),
            );
            let asset2 = AssetId::from_bytes(
                hex!("0200010700000000000000000000000000000000000000000000000000000000").into(),
            );
            let asset3 = AssetId::from_bytes(
                hex!("0200020700000000000000000000000000000000000000000000000000000000").into(),
            );
            let asset4 = AssetId::from_bytes(
                hex!("0200030700000000000000000000000000000000000000000000000000000000").into(),
            );
            let asset5 = AssetId::from_bytes(
                hex!("0200040700000000000000000000000000000000000000000000000000000000").into(),
            );
            let asset6 = AssetId::from_bytes(
                hex!("0200050700000000000000000000000000000000000000000000000000000000").into(),
            );
            let set: BTreeSet<AssetId> = vec![asset1, asset2, asset3].into_iter().collect();
            OldMarkerTokensIndex::<AssetId>::put(set);
            OldProperties::<AccountId, AssetId>::insert::<_, _, (AccountId, AccountId, AssetId)>(
                &asset1,
                &asset2,
                (ALICE(), ALICE(), asset3.clone()),
            );
            OldProperties::<AccountId, AssetId>::insert::<_, _, (AccountId, AccountId, AssetId)>(
                &asset4,
                &asset5,
                (BOB(), BOB(), asset6.clone()),
            );

            assets::Pallet::<Runtime>::register_asset_id(
                ALICE(),
                asset3.clone(),
                AssetSymbol(b"A".to_vec()),
                AssetName(b"B".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            )
            .unwrap();

            assets::Pallet::<Runtime>::mint_to(&asset3, &ALICE(), &ALICE(), balance!(3)).unwrap();
            assets::Pallet::<Runtime>::mint_to(&asset3, &ALICE(), &BOB(), balance!(3)).unwrap();

            assets::Pallet::<Runtime>::register_asset_id(
                BOB(),
                asset6.clone(),
                AssetSymbol(b"C".to_vec()),
                AssetName(b"D".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            )
            .unwrap();

            assets::Pallet::<Runtime>::mint_to(&asset6, &BOB(), &ALICE(), balance!(4)).unwrap();
            assets::Pallet::<Runtime>::mint_to(&asset6, &BOB(), &BOB(), balance!(4)).unwrap();

            super::migrate::<Runtime>();

            assert!(!OldMarkerTokensIndex::<AssetId>::exists());
            assert_eq!(
                Properties::<Runtime>::iter().collect::<Vec<_>>(),
                vec![
                    (asset1, asset2, (ALICE(), ALICE())),
                    (asset4, asset5, (BOB(), BOB())),
                ]
            );
            assert_eq!(TotalIssuances::<Runtime>::get(ALICE()), Some(balance!(6)));
            assert_eq!(TotalIssuances::<Runtime>::get(BOB()), Some(balance!(8)));

            assert_eq!(
                PoolProviders::<Runtime>::get(ALICE(), ALICE()),
                Some(balance!(3))
            );
            assert_eq!(
                PoolProviders::<Runtime>::get(ALICE(), BOB()),
                Some(balance!(3))
            );

            assert_eq!(
                PoolProviders::<Runtime>::get(BOB(), ALICE()),
                Some(balance!(4))
            );
            assert_eq!(
                PoolProviders::<Runtime>::get(BOB(), BOB()),
                Some(balance!(4))
            );
        });
    }
}
