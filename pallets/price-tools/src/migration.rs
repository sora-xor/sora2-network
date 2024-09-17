pub mod v2 {
    use frame_support::dispatch::GetStorageVersion;
    use frame_support::traits::StorageVersion;

    use crate::PriceInfos;
    use crate::{AggregatedPriceInfo, Pallet};
    use crate::{Config, PriceInfo};

    pub fn migrate<T: Config>() {
        if Pallet::<T>::on_chain_storage_version() < StorageVersion::new(2) {
            PriceInfos::<T>::translate::<PriceInfo, _>(
                |_, old_price_info| -> Option<AggregatedPriceInfo> {
                    Some(AggregatedPriceInfo {
                        buy: old_price_info,
                        sell: PriceInfo::default(),
                    })
                },
            );
            StorageVersion::new(2).put::<Pallet<T>>()
        }
    }
}
