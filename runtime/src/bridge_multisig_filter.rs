// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use codec::DecodeLimit;
use frame_support::traits::Contains;

use crate::{Runtime, RuntimeCall};

/// Bridge signatories may dispatch bridge operations without transaction fees.
/// Equivocation reports must use a signed, fee-paying route, including when a
/// report is nested within a call that otherwise qualifies for bridge exemption.
pub struct BridgeMultisigCallFilter;

impl BridgeMultisigCallFilter {
    const MAX_DEPTH: u32 = 32;

    fn permits_encoded(data: &[u8], depth: u32) -> bool {
        if depth >= Self::MAX_DEPTH {
            return false;
        }
        RuntimeCall::decode_all_with_depth_limit(Self::MAX_DEPTH - depth, &mut &data[..])
            .map(|call| Self::permits(&call, depth))
            .unwrap_or(false)
    }

    fn permits(call: &RuntimeCall, depth: u32) -> bool {
        if depth >= Self::MAX_DEPTH {
            return false;
        }
        let next = depth + 1;
        match call {
            RuntimeCall::Babe(
                pallet_babe::Call::report_equivocation { .. }
                | pallet_babe::Call::report_equivocation_unsigned { .. },
            )
            | RuntimeCall::Grandpa(
                pallet_grandpa::Call::report_equivocation { .. }
                | pallet_grandpa::Call::report_equivocation_unsigned { .. },
            ) => false,
            RuntimeCall::Utility(
                pallet_utility::Call::batch { calls }
                | pallet_utility::Call::batch_all { calls }
                | pallet_utility::Call::force_batch { calls },
            ) => calls.iter().all(|call| Self::permits(call, next)),
            RuntimeCall::Utility(
                pallet_utility::Call::as_derivative { call, .. }
                | pallet_utility::Call::dispatch_as { call, .. }
                | pallet_utility::Call::with_weight { call, .. }
                | pallet_utility::Call::dispatch_as_fallible { call, .. },
            )
            | RuntimeCall::Multisig(
                pallet_multisig::Call::as_multi { call, .. }
                | pallet_multisig::Call::as_multi_threshold_1 { call, .. },
            )
            | RuntimeCall::XorFee(xor_fee::Call::xorless_call { call, .. })
            | RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi_threshold_1 {
                call,
                ..
            })
            | RuntimeCall::Scheduler(
                pallet_scheduler::Call::schedule { call, .. }
                | pallet_scheduler::Call::schedule_named { call, .. }
                | pallet_scheduler::Call::schedule_after { call, .. }
                | pallet_scheduler::Call::schedule_named_after { call, .. },
            ) => Self::permits(call, next),
            RuntimeCall::Utility(pallet_utility::Call::if_else { main, fallback }) => {
                Self::permits(main, next) && Self::permits(fallback, next)
            }
            RuntimeCall::Council(
                pallet_collective::Call::execute { proposal, .. }
                | pallet_collective::Call::propose { proposal, .. },
            )
            | RuntimeCall::TechnicalCommittee(
                pallet_collective::Call::execute { proposal, .. }
                | pallet_collective::Call::propose { proposal, .. },
            ) => Self::permits(proposal, next),
            RuntimeCall::Council(pallet_collective::Call::close { proposal_hash, .. }) => {
                pallet_collective::ProposalOf::<Runtime, crate::CouncilCollective>::get(
                    proposal_hash,
                )
                .map(|call| Self::permits(&call, next))
                .unwrap_or(false)
            }
            RuntimeCall::TechnicalCommittee(pallet_collective::Call::close {
                proposal_hash,
                ..
            }) => pallet_collective::ProposalOf::<Runtime, crate::TechnicalCollective>::get(
                proposal_hash,
            )
            .map(|call| Self::permits(&call, next))
            .unwrap_or(false),
            RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi { call, .. }) => {
                Self::permits_encoded(call, next)
            }
            RuntimeCall::BridgeMultisig(bridge_multisig::Call::approve_as_multi {
                call_hash,
                ..
            }) => bridge_multisig::Pallet::<Runtime>::decode_call(call_hash, None)
                .map(|call| Self::permits(&call, next))
                .unwrap_or(false),
            #[cfg(feature = "private-net")]
            RuntimeCall::Sudo(
                pallet_sudo::Call::sudo { call }
                | pallet_sudo::Call::sudo_unchecked_weight { call, .. }
                | pallet_sudo::Call::sudo_as { call, .. },
            ) => Self::permits(call, next),
            _ => true,
        }
    }
}

impl Contains<RuntimeCall> for BridgeMultisigCallFilter {
    fn contains(call: &RuntimeCall) -> bool {
        Self::permits(call, 0)
    }
}

/// The legacy bridge multisig is a protocol executor. Its pending key contains
/// only the call hash, so accepting an arbitrary first timepoint lets one peer
/// prevent the remaining peers from approving the canonical operation.
pub struct BridgeMultisigOperationFilter;

impl bridge_multisig::OperationFilter<crate::AccountId, RuntimeCall, crate::BlockNumber>
    for BridgeMultisigOperationFilter
{
    fn validate(
        id: &crate::AccountId,
        timepoint: Option<bridge_multisig::BridgeTimepoint<crate::BlockNumber>>,
        call: Option<&RuntimeCall>,
    ) -> sp_runtime::DispatchResult {
        use frame_support::ensure;
        let Some(RuntimeCall::EthBridge(protocol)) = call else {
            return Err(frame_system::Error::<Runtime>::CallFiltered.into());
        };
        let (network, canonical) = match protocol {
            eth_bridge::Call::register_incoming_request { incoming_request } => {
                (incoming_request.network_id(), incoming_request.timepoint())
            }
            eth_bridge::Call::import_incoming_request {
                load_incoming_request,
                ..
            } => (
                load_incoming_request.network_id(),
                load_incoming_request.timepoint(),
            ),
            eth_bridge::Call::finalize_incoming_request { network_id, hash }
            | eth_bridge::Call::abort_request {
                network_id, hash, ..
            } => {
                let request = eth_bridge::Requests::<Runtime>::get(network_id, hash)
                    .ok_or(eth_bridge::Error::<Runtime>::UnknownRequest)?;
                (*network_id, request.timepoint())
            }
            // Outgoing approvals and cancellation votes are authenticated direct
            // peer calls; neither uses the bridge account's multisig dispatcher.
            _ => return Err(frame_system::Error::<Runtime>::CallFiltered.into()),
        };
        ensure!(
            crate::EthBridge::bridge_account(network).as_ref() == Some(id),
            frame_system::Error::<Runtime>::CallFiltered
        );
        ensure!(
            timepoint == Some(canonical),
            bridge_multisig::Error::<Runtime>::WrongTimepoint
        );
        ensure!(
            crate::EthBridge::validate_peer_protocol_call(id, protocol)?,
            frame_system::Error::<Runtime>::CallFiltered
        );
        Ok(())
    }
}
