// Reuse the bonded runtime fixture without changing the production test modules.
use super::*;

/// The first block in a BABE epoch belongs to the incoming session. Its BABE
/// authority index must therefore be resolved against the incoming validator
/// order, and its liveness credit must survive under that session's index.
///
/// This executes the actual generated runtime tuple, not a hand-written ordering
/// of Authorship and Session. In the pinned SDK, construct_runtime's
/// `decl_all_pallets` appends the pallet declarations in source order and the
/// tuple `OnInitialize` implementation invokes them in that order; pallet call
/// indices (Session=12, Authorship=16) do not determine hook order.
#[test]
fn first_new_session_block_credits_its_real_author_and_new_session() {
    use frame_support::traits::OnInitialize;
    use sp_consensus_babe::digests::{PreDigest, SecondaryPlainPreDigest};
    use sp_runtime::{generic::Digest, DigestItem};

    ext().execute_with(|| {
        let validators = Validators::new();

        let session_keys = validators
            .accounts
            .iter()
            .enumerate()
            .map(|(index, account)| {
                let seed = [index as u8 + 1; 32];
                let keys = crate::opaque::SessionKeys {
                    babe: sp_core::sr25519::Pair::from_seed(&seed).public().into(),
                    grandpa: sp_core::ed25519::Pair::from_seed(&seed).public().into(),
                    im_online: validators.keys[index].clone(),
                    beefy: sp_core::ecdsa::Pair::from_seed(&seed).public().into(),
                };
                pallet_session::NextKeys::<Runtime>::insert(account, &keys);
                (account.clone(), keys)
            })
            .collect::<Vec<_>>();

        // Epoch N's index zero is account 0; epoch N+1's index zero is account 1.
        // Install both announced sets so the pre-digest describes a block from
        // the incoming set rather than merely editing the current account list.
        let old_authorities = session_keys
            .iter()
            .map(|(_, keys)| (keys.babe.clone(), 1))
            .collect::<Vec<_>>();
        let mut incoming_keys = session_keys;
        incoming_keys.rotate_left(1);
        let incoming_authorities = incoming_keys
            .iter()
            .map(|(_, keys)| (keys.babe.clone(), 1))
            .collect::<Vec<_>>();
        pallet_babe::Authorities::<Runtime>::put(frame_support::WeakBoundedVec::<
            _,
            crate::MaxAuthorities,
        >::force_from(old_authorities, None));
        pallet_babe::NextAuthorities::<Runtime>::put(frame_support::WeakBoundedVec::<
            _,
            crate::MaxAuthorities,
        >::force_from(
            incoming_authorities, None
        ));
        pallet_session::QueuedKeys::<Runtime>::put(&incoming_keys);
        pallet_session::QueuedChanged::<Runtime>::put(true);

        // Only account zero lacks an old-session liveness record. A block from
        // account one in the NEW session must not retroactively make it online.
        for index in 1..validators.accounts.len() {
            validators.note_author(index);
        }
        pallet_staking::ErasRewardPoints::<Runtime>::remove(ERA);

        let new_session = FIRST_SESSION + 1;
        let first_new_slot = 1 + u64::from(new_session) * crate::EpochDuration::get();
        pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
        pallet_babe::EpochIndex::<Runtime>::put(u64::from(FIRST_SESSION));
        pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(first_new_slot - 1));
        pallet_babe::Initialized::<Runtime>::kill();

        let digest = Digest {
            logs: vec![DigestItem::PreRuntime(
                sp_consensus_babe::BABE_ENGINE_ID,
                PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
                    authority_index: 0,
                    slot: first_new_slot.into(),
                })
                .encode(),
            )],
        };
        System::reset_events();
        System::initialize(&2, &Default::default(), &digest);
        <crate::AllPalletsWithSystem as OnInitialize<crate::BlockNumber>>::on_initialize(2);

        // These guard assertions verify the actual Session hook rotated to the
        // intended BABE epoch and installed the queued, reordered validators.
        assert_eq!(Session::current_index(), new_session);
        assert_eq!(crate::Babe::epoch_index(), u64::from(new_session));
        assert_eq!(Session::validators()[0], validators.accounts[1]);

        let offline = System::events()
            .into_iter()
            .find_map(|event| match event.event {
                RuntimeEvent::ImOnline(pallet_im_online::Event::SomeOffline { offline }) => Some(
                    offline
                        .into_iter()
                        .map(|(account, _)| account)
                        .collect::<Vec<_>>(),
                ),
                RuntimeEvent::ImOnline(pallet_im_online::Event::AllGood) => Some(vec![]),
                _ => None,
            })
            .expect("the actual session-ending hook emitted its liveness assessment");
        let points = pallet_staking::ErasRewardPoints::<Runtime>::get(ERA);

        // Expected on the corrected runtime: account 1 is the current author,
        // has one NEW-session liveness credit and all 20 reward points; account
        // 0 remains offline in the old session. On the current runtime the
        // actual tuple instead yields account 0, zero new-session credits,
        // (20, 0) reward points, and an empty old-session offline list. Session's
        // end hook has already removed the misplaced old-session credit.
        assert_eq!(
            (
                crate::Authorship::author(),
                pallet_im_online::AuthoredBlocks::<Runtime>::get(
                    new_session,
                    &validators.accounts[1],
                ),
                points
                    .individual
                    .get(&validators.accounts[0])
                    .copied()
                    .unwrap_or(0),
                points
                    .individual
                    .get(&validators.accounts[1])
                    .copied()
                    .unwrap_or(0),
                offline,
                pallet_im_online::AuthoredBlocks::<Runtime>::iter_prefix(FIRST_SESSION).count(),
            ),
            (
                Some(validators.accounts[1].clone()),
                1,
                0,
                20,
                vec![validators.accounts[0].clone()],
                0,
            ),
            "session rotation must precede author resolution and liveness/reward credit",
        );
    });
}

/// Correct author/session ordering must not let old-session offline punishment
/// reject the first block that a recovering validator produces in the new one.
/// Run this after fixing the Authorship/Session order: the old order incorrectly
/// subtracts the returning author from the previous session's offline majority.
#[test]
#[cfg(not(feature = "private-net"))]
fn returning_boundary_author_survives_prior_session_offline_penalty() {
    use frame_support::traits::{OnFinalize, OnInitialize};
    use sp_consensus_babe::digests::{PreDigest, SecondaryPlainPreDigest};
    use sp_runtime::{generic::Digest, DigestItem};

    ext().execute_with(|| {
        let validators = Validators::new();
        let session_keys = validators
            .accounts
            .iter()
            .enumerate()
            .map(|(index, account)| {
                let seed = [index as u8 + 1; 32];
                let keys = crate::opaque::SessionKeys {
                    babe: sp_core::sr25519::Pair::from_seed(&seed).public().into(),
                    grandpa: sp_core::ed25519::Pair::from_seed(&seed).public().into(),
                    im_online: validators.keys[index].clone(),
                    beefy: sp_core::ecdsa::Pair::from_seed(&seed).public().into(),
                };
                pallet_session::NextKeys::<Runtime>::insert(account, &keys);
                (account.clone(), keys)
            })
            .collect::<Vec<_>>();
        let authorities = frame_support::WeakBoundedVec::<_, crate::MaxAuthorities>::force_from(
            session_keys
                .iter()
                .map(|(_, keys)| (keys.babe.clone(), 1))
                .collect::<Vec<_>>(),
            None,
        );
        pallet_babe::Authorities::<Runtime>::put(&authorities);
        pallet_babe::NextAuthorities::<Runtime>::put(&authorities);
        pallet_session::QueuedKeys::<Runtime>::put(&session_keys);
        // A normal intra-era rotation retains disablements when the validator
        // identities and keys are unchanged.
        pallet_session::QueuedChanged::<Runtime>::put(false);

        const MAJORITY: usize = 13;
        const RETURNING_AUTHOR: usize = MAJORITY - 1;
        for index in MAJORITY..validators.accounts.len() {
            validators.note_author(index);
        }

        let new_session = FIRST_SESSION + 1;
        let first_new_slot = 1 + u64::from(new_session) * crate::EpochDuration::get();
        pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
        pallet_babe::EpochIndex::<Runtime>::put(u64::from(FIRST_SESSION));
        pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(first_new_slot - 1));
        pallet_babe::Initialized::<Runtime>::kill();
        let digest = Digest {
            logs: vec![DigestItem::PreRuntime(
                sp_consensus_babe::BABE_ENGINE_ID,
                PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
                    authority_index: RETURNING_AUTHOR as u32,
                    slot: first_new_slot.into(),
                })
                .encode(),
            )],
        };
        System::reset_events();
        System::initialize(&2, &Default::default(), &digest);
        <crate::AllPalletsWithSystem as OnInitialize<crate::BlockNumber>>::on_initialize(2);

        assert_eq!(Session::current_index(), new_session);
        assert_eq!(
            pallet_im_online::AuthoredBlocks::<Runtime>::get(
                new_session,
                &validators.accounts[RETURNING_AUTHOR],
            ),
            1,
            "the new-session block is evidence this validator has returned",
        );
        assert_eq!(
            queued_slash_count(),
            MAJORITY,
            "the real majority outage still receives its monetary penalties",
        );
        assert!(System::events().iter().any(|event| matches!(
            &event.event,
            RuntimeEvent::ImOnline(pallet_im_online::Event::SomeOffline { offline })
                if offline.len() == MAJORITY
                    && offline.iter().any(|(account, _)|
                        account == &validators.accounts[RETURNING_AUTHOR])
        )));

        // This is the actual BABE finalization check, not a reconstructed
        // is_disabled predicate. The current Staking route disables the returning
        // author while processing the ending session, then BABE rejects its new
        // block with "Validator with index 12 is disabled ...". Capture the panic
        // so the desired acceptance assertion reports the concrete failure.
        let finalized = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <crate::Babe as OnFinalize<crate::BlockNumber>>::on_finalize(2);
        }));
        let failure = finalized.as_ref().err().map(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|message| (*message).to_owned()))
                .unwrap_or_else(|| "unknown panic payload".to_owned())
        });
        assert!(
            finalized.is_ok() && Session::disabled_validators().is_empty(),
            "offline monetary penalties must preserve recovery block validity: failure={failure:?}, disabled={:?}",
            Session::disabled_validators(),
        );
    });
}
