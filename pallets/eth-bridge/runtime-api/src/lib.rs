#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use sp_runtime::DispatchError;
use sp_std::prelude::*;

sp_api::decl_runtime_apis! {
    pub trait EthBridgeRuntimeApi<
        Hash,
        Approval,
        AccountId,
        AssetKind,
        AssetId,
        Address,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
> where
        Hash: Codec,
        Approval: Codec,
        AccountId: Codec,
        AssetKind: Codec,
        AssetId: Codec,
        Address: Codec,
        OffchainRequest: Codec,
        RequestStatus: Codec,
        OutgoingRequestEncoded: Codec,
    {
        fn get_requests(hashes: Vec<Hash>) -> Result<Vec<(OffchainRequest, RequestStatus)>, DispatchError>;
        fn get_approved_requests(hashes: Vec<Hash>) -> Result<Vec<(OutgoingRequestEncoded, Vec<Approval>)>, DispatchError>;
        fn get_approvals(hashes: Vec<Hash>) -> Result<Vec<Vec<Approval>>, DispatchError>;
        fn get_account_requests(account_id: AccountId, status_filter: Option<RequestStatus>) -> Result<Vec<Hash>, DispatchError>;
        fn get_registered_assets() -> Result<Vec<(AssetKind, AssetId, Option<Address>)>, DispatchError>;
    }
}
