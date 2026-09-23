// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{Babe, Runtime, RuntimeOrigin, System};
use codec::{Decode, Encode};
use frame_support::{
    assert_ok,
    traits::{OnInitialize, OnRuntimeUpgrade},
};
use framenode_chain_spec::ext;
use sp_consensus_babe::digests::{NextConfigDescriptor, PreDigest, SecondaryPlainPreDigest};
use sp_consensus_babe::{AllowedSlots, BabeConfiguration, BabeEpochConfiguration, Epoch};
use sp_runtime::{Digest, DigestItem};

fn legacy() -> BabeEpochConfiguration {
    BabeEpochConfiguration {
        c: (1, 4),
        allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
    }
}

fn plain() -> BabeEpochConfiguration {
    BabeEpochConfiguration {
        c: (1, 4),
        allowed_slots: AllowedSlots::PrimaryAndSecondaryPlainSlots,
    }
}

fn descriptor(config: BabeEpochConfiguration) -> NextConfigDescriptor {
    NextConfigDescriptor::V1 {
        c: config.c,
        allowed_slots: config.allowed_slots,
    }
}

fn install_mainnet_legacy_epoch() {
    frame_system::BlockHash::<Runtime>::insert(0, crate::babe_config::MAINNET_GENESIS_HASH);
    System::set_block_number(10);
    pallet_babe::EpochConfig::<Runtime>::put(legacy());
    pallet_babe::NextEpochConfig::<Runtime>::put(legacy());
    pallet_babe::PendingEpochConfigChange::<Runtime>::kill();
    pallet_babe::EpochIndex::<Runtime>::put(100u64);
    pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1u64));
    pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(
        1 + 100 * crate::EpochDuration::get() + 10,
    ));
    pallet_babe::Randomness::<Runtime>::put([41u8; 32]);
    pallet_babe::NextRandomness::<Runtime>::put([42u8; 32]);
    let authorities = pallet_babe::Authorities::<Runtime>::get();
    assert!(
        !authorities.is_empty(),
        "genesis fixture has genuine BABE authorities"
    );
    pallet_babe::NextAuthorities::<Runtime>::put(authorities);
    pallet_babe::Initialized::<Runtime>::kill();
    assert!(!crate::babe_config::repair_attempted());
}

fn repair() {
    crate::babe_config::ScheduleMainnetPlainEpochConfig::on_runtime_upgrade();
}

/// Run BABE's actual block initialization and epoch transition, with the same
/// authority set in the next epoch. This isolates the consensus protocol from
/// elections; no configuration storage or consensus digest is edited here.
fn next_boundary() -> Vec<sp_consensus_babe::ConsensusLog> {
    let epoch = Babe::epoch_index() + 1;
    let slot = u64::from(Babe::genesis_slot()) + epoch * crate::EpochDuration::get();
    let block = System::block_number() + 1;
    pallet_babe::Initialized::<Runtime>::kill();
    System::initialize(
        &block,
        &Default::default(),
        &Digest {
            logs: vec![DigestItem::PreRuntime(
                sp_consensus_babe::BABE_ENGINE_ID,
                PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
                    authority_index: 0,
                    slot: slot.into(),
                })
                .encode(),
            )],
        },
    );
    <Babe as OnInitialize<crate::BlockNumber>>::on_initialize(block);
    let authorities = pallet_babe::Authorities::<Runtime>::get();
    Babe::enact_epoch_change(authorities.clone(), authorities, Some(epoch as u32));
    assert_eq!(Babe::epoch_index(), epoch);
    System::digest()
        .logs
        .into_iter()
        .filter_map(|item| match item {
            DigestItem::Consensus(id, data) if id == sp_consensus_babe::BABE_ENGINE_ID => {
                Some(sp_consensus_babe::ConsensusLog::decode(&mut data.as_slice()).unwrap())
            }
            _ => None,
        })
        .collect()
}

fn configuration() -> BabeConfiguration {
    BabeConfiguration::decode(
        &mut crate::api::dispatch("BabeApi_configuration", &[])
            .expect("the actual exported BABE configuration API exists")
            .as_slice(),
    )
    .expect("the API returns SCALE BabeConfiguration")
}

fn current_epoch() -> Epoch {
    Epoch::decode(
        &mut crate::api::dispatch("BabeApi_current_epoch", &[])
            .expect("the actual exported BABE current-epoch API exists")
            .as_slice(),
    )
    .expect("the API returns SCALE Epoch")
}

#[test]
fn configuration_uses_active_plain_slot_mode_and_probability() {
    ext().execute_with(|| {
        let active = BabeEpochConfiguration {
            c: (1, 5),
            allowed_slots: AllowedSlots::PrimaryAndSecondaryPlainSlots,
        };
        pallet_babe::EpochConfig::<Runtime>::put(&active);
        let actual = configuration();
        assert_eq!(
            (actual.c, actual.allowed_slots),
            (active.c, active.allowed_slots),
            "configuration() must agree with the active epoch, including after repair",
        );
        assert_eq!(actual.authorities, Babe::authorities().to_vec());
        assert_eq!(actual.randomness, Babe::randomness());
        assert_eq!(current_epoch().config, active);
    });
}

#[test]
fn configuration_preserves_future_vrf_mode_and_nondefault_probability() {
    ext().execute_with(|| {
        let active = BabeEpochConfiguration {
            c: (2, 7),
            allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
        };
        pallet_babe::EpochConfig::<Runtime>::put(&active);
        let actual = configuration();
        assert_eq!(
            (actual.c, actual.allowed_slots),
            (active.c, active.allowed_slots)
        );
        assert_eq!(current_epoch().config, active);
    });
}

#[test]
fn configuration_uses_genesis_fallback_before_epoch_storage_exists() {
    ext().execute_with(|| {
        pallet_babe::EpochConfig::<Runtime>::kill();
        let actual = configuration();
        let genesis = crate::constants::BABE_GENESIS_EPOCH_CONFIG;
        assert_eq!(
            (actual.c, actual.allowed_slots),
            (genesis.c, genesis.allowed_slots)
        );
    });
}

#[test]
fn mainnet_repair_announces_then_activates_without_silent_epoch_rewrite() {
    ext().execute_with(|| {
        install_mainnet_legacy_epoch();
        let before = (Babe::authorities(), pallet_babe::NextAuthorities::<Runtime>::get(),
            Babe::randomness(), pallet_babe::NextRandomness::<Runtime>::get(),
            Babe::epoch_index(), Babe::genesis_slot(), Babe::current_slot(), System::digest());
        repair();
        assert_eq!(pallet_babe::EpochConfig::<Runtime>::get(), Some(legacy()));
        assert_eq!(pallet_babe::NextEpochConfig::<Runtime>::get(), Some(legacy()));
        assert_eq!(pallet_babe::PendingEpochConfigChange::<Runtime>::get(), Some(descriptor(plain())));
        assert!(crate::babe_config::repair_attempted());
        assert_eq!((Babe::authorities(), pallet_babe::NextAuthorities::<Runtime>::get(),
            Babe::randomness(), pallet_babe::NextRandomness::<Runtime>::get(),
            Babe::epoch_index(), Babe::genesis_slot(), Babe::current_slot(), System::digest()), before,
            "migration must not rewrite consensus state or emit an out-of-boundary digest");

        let first = next_boundary();
        assert!(first.iter().any(|d| matches!(d, sp_consensus_babe::ConsensusLog::NextConfigData(c) if *c == descriptor(plain()))));
        assert_eq!(current_epoch().config, legacy());
        assert_eq!(pallet_babe::NextEpochConfig::<Runtime>::get(), Some(plain()));
        assert_eq!(pallet_babe::PendingEpochConfigChange::<Runtime>::get(), None);
        assert_eq!(Babe::randomness(), before.3, "normal epoch randomness promotion is preserved");
        let next_randomness = pallet_babe::NextRandomness::<Runtime>::get();

        let second = next_boundary();
        assert!(!second.iter().any(|d| matches!(d, sp_consensus_babe::ConsensusLog::NextConfigData(_))));
        assert_eq!(current_epoch().config, plain());
        assert_eq!(pallet_babe::NextEpochConfig::<Runtime>::get(), Some(plain()));
        assert_eq!(configuration().allowed_slots, AllowedSlots::PrimaryAndSecondaryPlainSlots);
        assert_eq!(Babe::randomness(), next_randomness);
        assert_eq!(Babe::authorities(), before.0);
    });
}

#[test]
fn mainnet_repair_does_not_override_an_existing_governance_plan() {
    for pending in [
        plain(),
        BabeEpochConfiguration {
            c: (2, 7),
            allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
        },
    ] {
        ext().execute_with(|| {
            install_mainnet_legacy_epoch();
            let plan = descriptor(pending);
            assert_ok!(Babe::plan_config_change(
                RuntimeOrigin::root(),
                plan.clone()
            ));
            repair();
            assert_eq!(pallet_babe::EpochConfig::<Runtime>::get(), Some(legacy()));
            assert_eq!(
                pallet_babe::NextEpochConfig::<Runtime>::get(),
                Some(legacy())
            );
            assert_eq!(
                pallet_babe::PendingEpochConfigChange::<Runtime>::get(),
                Some(plan)
            );
            assert!(crate::babe_config::repair_attempted());
            pallet_babe::PendingEpochConfigChange::<Runtime>::kill();
            repair();
            assert_eq!(
                pallet_babe::PendingEpochConfigChange::<Runtime>::get(),
                None,
                "a later upgrade must not turn a previously skipped repair into a new plan"
            );
        });
    }
}

#[test]
fn mainnet_repair_preserves_unknown_or_missing_epoch_configurations() {
    for (current, next) in [
        (None, Some(legacy())),
        (Some(legacy()), None),
        (Some(plain()), Some(legacy())),
        (Some(legacy()), Some(plain())),
        (Some(plain()), Some(plain())),
        (
            Some(BabeEpochConfiguration {
                c: (1, 5),
                allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
            }),
            Some(legacy()),
        ),
        (
            Some(BabeEpochConfiguration {
                c: (2, 7),
                allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
            }),
            Some(BabeEpochConfiguration {
                c: (2, 7),
                allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
            }),
        ),
    ] {
        ext().execute_with(|| {
            install_mainnet_legacy_epoch();
            pallet_babe::EpochConfig::<Runtime>::set(current.clone());
            pallet_babe::NextEpochConfig::<Runtime>::set(next.clone());
            repair();
            assert_eq!(pallet_babe::EpochConfig::<Runtime>::get(), current);
            assert_eq!(pallet_babe::NextEpochConfig::<Runtime>::get(), next);
            assert_eq!(
                pallet_babe::PendingEpochConfigChange::<Runtime>::get(),
                None
            );
            assert!(crate::babe_config::repair_attempted());
            pallet_babe::EpochConfig::<Runtime>::put(legacy());
            pallet_babe::NextEpochConfig::<Runtime>::put(legacy());
            repair();
            assert_eq!(
                pallet_babe::PendingEpochConfigChange::<Runtime>::get(),
                None
            );
        });
    }
}

#[test]
fn repair_leaves_other_networks_and_their_genesis_mode_unchanged() {
    ext().execute_with(|| {
        install_mainnet_legacy_epoch();
        frame_system::BlockHash::<Runtime>::insert(0, sp_core::H256::repeat_byte(99));
        repair();
        assert!(!crate::babe_config::repair_attempted());
        assert_eq!(pallet_babe::EpochConfig::<Runtime>::get(), Some(legacy()));
        assert_eq!(
            pallet_babe::NextEpochConfig::<Runtime>::get(),
            Some(legacy())
        );
        assert_eq!(
            pallet_babe::PendingEpochConfigChange::<Runtime>::get(),
            None
        );
        assert_eq!(
            configuration().allowed_slots,
            AllowedSlots::PrimaryAndSecondaryVRFSlots
        );
    });
}

#[test]
fn repeated_upgrade_preserves_a_future_legitimate_vrf_transition() {
    for future in [
        legacy(),
        BabeEpochConfiguration {
            c: (2, 7),
            allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
        },
    ] {
        ext().execute_with(|| {
            install_mainnet_legacy_epoch();
            repair();
            next_boundary();
            next_boundary();
            assert_ok!(Babe::plan_config_change(
                RuntimeOrigin::root(),
                descriptor(future.clone())
            ));
            next_boundary();
            next_boundary();
            assert_eq!(current_epoch().config, future);
            repair();
            let actual = configuration();
            assert_eq!(
                (actual.c, actual.allowed_slots),
                (future.c, future.allowed_slots)
            );
            assert_eq!(
                pallet_babe::PendingEpochConfigChange::<Runtime>::get(),
                None
            );
            assert!(crate::babe_config::repair_attempted());
        });
    }
}

#[cfg(feature = "try-runtime")]
#[test]
fn migration_pre_and_post_checks_cover_scheduled_and_preserved_states() {
    use crate::babe_config::ScheduleMainnetPlainEpochConfig as Migration;
    for case in [
        "legacy",
        "pending",
        "already-attempted",
        "aligned",
        "other-network",
    ] {
        ext().execute_with(|| {
            install_mainnet_legacy_epoch();
            match case {
                "pending" => {
                    assert_ok!(Babe::plan_config_change(
                        RuntimeOrigin::root(),
                        descriptor(BabeEpochConfiguration {
                            c: (2, 7),
                            allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
                        }),
                    ));
                }
                "already-attempted" => repair(),
                "aligned" => {
                    pallet_babe::EpochConfig::<Runtime>::put(plain());
                    pallet_babe::NextEpochConfig::<Runtime>::put(plain());
                }
                "other-network" => {
                    frame_system::BlockHash::<Runtime>::insert(0, sp_core::H256::repeat_byte(99))
                }
                _ => {}
            }
            let before = Migration::pre_upgrade().expect("valid state encodes for try-runtime");
            repair();
            assert_ok!(Migration::post_upgrade(before));
        });
    }
}

#[cfg(feature = "try-runtime")]
#[test]
fn migration_postcheck_rejects_an_unannounced_active_configuration_rewrite() {
    use crate::babe_config::ScheduleMainnetPlainEpochConfig as Migration;
    ext().execute_with(|| {
        install_mainnet_legacy_epoch();
        let before = Migration::pre_upgrade().unwrap();
        repair();
        // Simulate the unsafe alternative: switching stored active mode before
        // the consensus announcement has been observed by persistent clients.
        pallet_babe::EpochConfig::<Runtime>::put(plain());
        assert!(Migration::post_upgrade(before).is_err());
    });
}
