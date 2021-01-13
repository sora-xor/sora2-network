use crate::{mock::*, Error};
use common::{balance::Balance, fixed, Fixed};
use frame_support::assert_noop;
use traits::MultiCurrency;

#[test]
fn subscribe_with_default_frequency_should_pass() {
    let mut ext = ExtBuilder::uninitialized().build();
    ext.execute_with(|| {
        PswapDistrModule::subscribe(FEES_ACCOUNT_A, DEX_A_ID, PoolTokenAId::get(), None)
            .expect("Failed to subscribe account.");
        assert_eq!(
            PswapDistrModule::subscribed_accounts(FEES_ACCOUNT_A),
            Some((
                DEX_A_ID,
                PoolTokenAId::get(),
                GetDefaultSubscriptionFrequency::get(),
                0
            ))
        );
    })
}

#[test]
fn subscribe_with_zero_frequency_should_fail() {
    let mut ext = ExtBuilder::uninitialized().build();
    ext.execute_with(|| {
        assert_noop!(
            PswapDistrModule::subscribe(FEES_ACCOUNT_A, DEX_A_ID, PoolTokenAId::get(), Some(0)),
            Error::<Runtime>::InvalidFrequency
        );
    })
}

#[test]
fn subscribe_with_existing_account_should_fail() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            PswapDistrModule::subscribe(FEES_ACCOUNT_A, DEX_A_ID, PoolTokenAId::get(), None),
            Error::<Runtime>::SubscriptionActive
        );
    })
}

#[test]
fn unsubscribe_with_inexistent_account_should_fail() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = PswapDistrModule::unsubscribe(1000);
        assert_noop!(result, Error::<Runtime>::UnknownSubscription);
    });
}

#[test]
fn distribute_existing_pswap_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let tech_account_id = GetPswapDistributionAccountId::get();
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_A,
            &DEX_A_ID,
            &PoolTokenAId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");

        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
            .expect("Failed to claim.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Failed to claim.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
            .expect("Failed to claim.");

        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        assert_eq!(balance_a, Balance::from(fixed!(3, 000000000000000000)));
        assert_eq!(balance_b, Balance::from(fixed!(2, 000000000000000000)));
        assert_eq!(balance_c, Balance::from(fixed!(1, 000000000000000000)));
    })
}

#[test]
fn distribute_with_zero_balance_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let tech_account_id = GetPswapDistributionAccountId::get();
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_B,
            &DEX_A_ID,
            &PoolTokenBId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");

        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A)),
            Error::<Runtime>::ZeroClaimableIncentives
        );
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B)),
            Error::<Runtime>::ZeroClaimableIncentives
        );
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C)),
            Error::<Runtime>::ZeroClaimableIncentives
        );

        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        assert_eq!(balance_a, Balance::from(0u128));
        assert_eq!(balance_b, Balance::from(0u128));
        assert_eq!(balance_c, Balance::from(0u128));
    })
}

#[test]
fn incentive_distribution_routine_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        for i in 0u64..5 {
            PswapDistrModule::incentive_distribution_routine(i);
        }

        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        assert_eq!(balance_a, Balance::from(0u128));
        assert_eq!(balance_b, Balance::from(0u128));
        assert_eq!(balance_c, Balance::from(0u128));

        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
            .expect("Failed to claim.");
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        assert_eq!(balance_a, Balance::from(3u128));
        assert_eq!(balance_b, Balance::from(0u128));
        assert_eq!(balance_c, Balance::from(0u128));

        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Failed to claim.");
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        assert_eq!(balance_a, Balance::from(3u128));
        assert_eq!(balance_b, Balance::from(2u128));
        assert_eq!(balance_c, Balance::from(0u128));

        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
            .expect("Failed to claim.");
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        assert_eq!(balance_a, Balance::from(3u128));
        assert_eq!(balance_b, Balance::from(2u128));
        assert_eq!(balance_c, Balance::from(1u128));

        for i in 5u64..10 {
            PswapDistrModule::incentive_distribution_routine(i);
        }

        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        assert_eq!(balance_a, Balance::from(3u128));
        assert_eq!(balance_b, Balance::from(2u128));
        assert_eq!(balance_c, Balance::from(1u128));
    })
}

#[test]
fn increasing_burn_rate_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(fixed!(0), PswapDistrModule::burn_rate());
        for i in 0u64..3 {
            PswapDistrModule::burn_rate_update_routine(i);
        }
        assert_eq!(fixed!(0, 1), PswapDistrModule::burn_rate());
        for i in 3u64..6 {
            PswapDistrModule::burn_rate_update_routine(i);
        }
        assert_eq!(fixed!(0, 2), PswapDistrModule::burn_rate());
        for i in 6u64..9 {
            PswapDistrModule::burn_rate_update_routine(i);
        }
        assert_eq!(fixed!(0, 3), PswapDistrModule::burn_rate());
        // Observe flatline
        for i in 9u64..12 {
            PswapDistrModule::burn_rate_update_routine(i);
        }
        assert_eq!(fixed!(0, 3), PswapDistrModule::burn_rate());
        for i in 9u64..1000 {
            PswapDistrModule::burn_rate_update_routine(i);
        }
        assert_eq!(fixed!(0, 3), PswapDistrModule::burn_rate());
    })
}

#[test]
fn claim_until_zero_should_pass() {
    let mut ext = ExtBuilder::with_accounts(vec![
        (
            LIQUIDITY_PROVIDER_A,
            PoolTokenAId::get(),
            Balance(fixed!(3)),
        ),
        (
            LIQUIDITY_PROVIDER_B,
            PoolTokenAId::get(),
            Balance(fixed!(2)),
        ),
        (
            LIQUIDITY_PROVIDER_C,
            PoolTokenAId::get(),
            Balance(fixed!(1)),
        ),
    ])
    .build();
    ext.execute_with(|| {
        let tech_account_id = GetPswapDistributionAccountId::get();

        // start with empty fees account, claiming should fail
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_A,
            &DEX_A_ID,
            &PoolTokenAId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A)),
            Error::<Runtime>::ZeroClaimableIncentives
        );
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(balance_a, Balance::from(0u128));
        assert_eq!(balance_b, Balance::from(0u128));
        assert_eq!(balance_c, Balance::from(0u128));
        assert_eq!(balance_d, Balance::from(0u128));

        // new pswap was derived from exchange, it should be claimable after distribution
        Assets::mint(
            Origin::signed(tech_account_id.clone()),
            GetIncentiveAssetId::get(),
            FEES_ACCOUNT_A,
            Balance::from(60u128),
        )
        .expect("Minting tokens is not expected to fail.");
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_A,
            &DEX_A_ID,
            &PoolTokenAId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
            .expect("Claiming is not expected to fail.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Claiming is not expected to fail.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
            .expect("Claiming is not expected to fail.");
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(balance_a, Balance::from(30u128));
        assert_eq!(balance_b, Balance::from(20u128));
        assert_eq!(balance_c, Balance::from(10u128));
        assert_eq!(balance_d, Balance::from(0u128));

        // again period of no incentives, should return error for non claimable
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_A,
            &DEX_A_ID,
            &PoolTokenAId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A)),
            Error::<Runtime>::ZeroClaimableIncentives
        );
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B)),
            Error::<Runtime>::ZeroClaimableIncentives
        );
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C)),
            Error::<Runtime>::ZeroClaimableIncentives
        );
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(balance_a, Balance::from(30u128));
        assert_eq!(balance_b, Balance::from(20u128));
        assert_eq!(balance_c, Balance::from(10u128));
        assert_eq!(balance_d, Balance::from(0u128));

        // new pswap was derived from exchange, it should be claimable after distribution, now only one account claims it
        Assets::mint(
            Origin::signed(tech_account_id.clone()),
            GetIncentiveAssetId::get(),
            FEES_ACCOUNT_A,
            Balance::from(600u128),
        )
        .expect("Minting tokens is not expected to fail.");
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_A,
            &DEX_A_ID,
            &PoolTokenAId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Claiming is not expected to fail.");
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(balance_a, Balance::from(30u128));
        assert_eq!(balance_b, Balance::from(220u128));
        assert_eq!(balance_c, Balance::from(10u128));
        assert_eq!(balance_d, Balance::from(400u128));

        // final pswap arrival, should be consistent for previously claimed and unclaimed
        Assets::mint(
            Origin::signed(tech_account_id.clone()),
            GetIncentiveAssetId::get(),
            FEES_ACCOUNT_A,
            Balance::from(6000u128),
        )
        .expect("Minting tokens is not expected to fail.");
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_A,
            &DEX_A_ID,
            &PoolTokenAId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
            .expect("Claiming is not expected to fail.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Claiming is not expected to fail.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
            .expect("Claiming is not expected to fail.");
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(balance_a, Balance::from(3330u128));
        assert_eq!(balance_b, Balance::from(2220u128));
        assert_eq!(balance_c, Balance::from(1110u128));
        assert_eq!(balance_d, Balance::from(0u128));
    })
}

#[test]
fn external_transfer_to_tech_account_after_distribution() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let tech_account_id = GetPswapDistributionAccountId::get();

        // initial distribution happens normally
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_A,
            &DEX_A_ID,
            &PoolTokenAId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");

        let balance_tech = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(balance_tech, Balance::from(fixed!(6, 000000000000000000)));

        // before claim, external transfer happens, therefore claimable value will be increased
        Assets::mint(
            Origin::signed(tech_account_id.clone()),
            GetIncentiveAssetId::get(),
            tech_account_id.clone(),
            Balance::from(fixed!(11111, 111111111111111111)),
        )
        .expect("Minting tokens is not expected to fail.");

        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
            .expect("Failed to claim.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Failed to claim.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
            .expect("Failed to claim.");

        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_tech = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        // externally added incentive is evenly distributed amoung current unclaimed balances
        assert_eq!(balance_a, Balance::from(fixed!(5558, 555555555558972008)));
        assert_eq!(balance_b, Balance::from(fixed!(3705, 703703703705981338)));
        assert_eq!(balance_c, Balance::from(fixed!(1852, 851851851846157765)));
        assert_eq!(
            balance_a + balance_b + balance_c,
            Balance::from(fixed!(11117, 111111111111111111))
        );
        assert_eq!(balance_tech, Balance::from(fixed!(0)));
    })
}

#[test]
fn jump_start_with_unowned_incentive_should_pass() {
    let mut ext = ExtBuilder::with_accounts(vec![
        (FEES_ACCOUNT_A, common::PSWAP.into(), Balance(fixed!(6))),
        (
            LIQUIDITY_PROVIDER_A,
            PoolTokenAId::get(),
            Balance(fixed!(3)),
        ),
        (
            LIQUIDITY_PROVIDER_B,
            PoolTokenAId::get(),
            Balance(fixed!(2)),
        ),
        (
            LIQUIDITY_PROVIDER_C,
            PoolTokenAId::get(),
            Balance(fixed!(1)),
        ),
    ])
    .build();
    ext.execute_with(|| {
        let tech_account_id = GetPswapDistributionAccountId::get();

        // initially no liquidity providers have received incentives yet, thus shares are not calculated for them yet,
        // however some incentive is transferred to claimable reserve
        Assets::mint(
            Origin::signed(tech_account_id.clone()),
            GetIncentiveAssetId::get(),
            tech_account_id.clone(),
            Balance::from(fixed!(11111, 111111111111111111)),
        )
        .expect("Minting tokens is not expected to fail.");

        // no one can claim it as shares are not calculated for this transfer
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A)),
            Error::<Runtime>::ZeroClaimableIncentives
        );
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B)),
            Error::<Runtime>::ZeroClaimableIncentives
        );
        assert_noop!(
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C)),
            Error::<Runtime>::ZeroClaimableIncentives
        );

        // now liquidity providers receive their incentive, and claim it
        PswapDistrModule::distribute_incentive(
            &FEES_ACCOUNT_A,
            &DEX_A_ID,
            &PoolTokenAId::get(),
            &tech_account_id,
        )
        .expect("Error is not expected during distribution");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
            .expect("Failed to claim.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Failed to claim.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
            .expect("Failed to claim.");
        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);

        // first claimer collects unowned incentive, special correction is applied so precision loss is avoided on following claims
        assert_eq!(balance_a, Balance::from(fixed!(11114, 111111111111111111)));
        assert_eq!(balance_b, Balance::from(fixed!(2, 000000000000000000)));
        assert_eq!(balance_c, Balance::from(fixed!(1, 000000000000000000)));
        assert_eq!(balance_d, Balance::from(fixed!(0, 000000000000000000)));
    })
}

#[test]
fn increasing_volumes_should_pass() {
    let mut ext = ExtBuilder::with_accounts(vec![
        (
            LIQUIDITY_PROVIDER_A,
            PoolTokenAId::get(),
            Balance(fixed!(3)),
        ),
        (
            LIQUIDITY_PROVIDER_B,
            PoolTokenAId::get(),
            Balance(fixed!(2)),
        ),
        (
            LIQUIDITY_PROVIDER_C,
            PoolTokenAId::get(),
            Balance(fixed!(1)),
        ),
    ])
    .build();
    ext.execute_with(|| {
        let tech_account_id = GetPswapDistributionAccountId::get();

        let mut decimals_factor = 1u128;

        for _ in 0..=27u32 {
            Assets::mint(
                Origin::signed(tech_account_id.clone()),
                GetIncentiveAssetId::get(),
                FEES_ACCOUNT_A,
                Balance::from(Fixed::from_inner(6u128 * decimals_factor)),
            )
            .expect("Minting tokens is not expected to fail.");
            PswapDistrModule::distribute_incentive(
                &FEES_ACCOUNT_A,
                &DEX_A_ID,
                &PoolTokenAId::get(),
                &tech_account_id,
            )
            .expect("Error is not expected during distribution");
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
                .expect("Claiming is not expected to fail.");
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
                .expect("Claiming is not expected to fail.");
            PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
                .expect("Claiming is not expected to fail.");
            decimals_factor *= 10u128;
        }

        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(
            balance_a,
            Balance::from(fixed!(3333333333, 333333333333333333))
        );
        assert_eq!(
            balance_b,
            Balance::from(fixed!(2222222222, 222222222222222222))
        );
        assert_eq!(
            balance_c,
            Balance::from(fixed!(1111111111, 111111111111111111))
        );
        assert_eq!(
            balance_a + balance_b + balance_c,
            Balance::from(fixed!(6666666666, 666666666666666666))
        );
        assert_eq!(balance_d, Balance::from(0u32));
    })
}

#[test]
fn multiple_pools_should_pass() {
    let mut ext = ExtBuilder::with_accounts(vec![
        (FEES_ACCOUNT_A, common::PSWAP.into(), Balance(fixed!(20))),
        (FEES_ACCOUNT_B, common::PSWAP.into(), Balance(fixed!(2))),
        (
            LIQUIDITY_PROVIDER_A,
            PoolTokenAId::get(),
            Balance(fixed!(2)),
        ),
        (
            LIQUIDITY_PROVIDER_B,
            PoolTokenBId::get(),
            Balance(fixed!(10)),
        ),
        (
            LIQUIDITY_PROVIDER_C,
            PoolTokenBId::get(),
            Balance(fixed!(10)),
        ),
    ])
    .build();
    ext.execute_with(|| {
        let tech_account_id = GetPswapDistributionAccountId::get();

        for i in 0u64..5 {
            PswapDistrModule::incentive_distribution_routine(i);
        }

        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
            .expect("Claiming is not expected to fail.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Claiming is not expected to fail.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
            .expect("Claiming is not expected to fail.");

        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(balance_a, Balance::from(20u128));
        assert_eq!(balance_b, Balance::from(1u128));
        assert_eq!(balance_c, Balance::from(1u128));
        assert_eq!(balance_d, Balance::from(0u128));
    })
}

#[test]
fn mixed_multiple_pools_should_pass() {
    let mut ext = ExtBuilder::with_accounts(vec![
        (FEES_ACCOUNT_A, common::PSWAP.into(), Balance(fixed!(20))),
        (FEES_ACCOUNT_B, common::PSWAP.into(), Balance(fixed!(4))),
        (
            LIQUIDITY_PROVIDER_A,
            PoolTokenAId::get(),
            Balance(fixed!(2)),
        ),
        (
            LIQUIDITY_PROVIDER_A,
            PoolTokenBId::get(),
            Balance(fixed!(10)),
        ),
        (
            LIQUIDITY_PROVIDER_B,
            PoolTokenBId::get(),
            Balance(fixed!(10)),
        ),
        (
            LIQUIDITY_PROVIDER_C,
            PoolTokenAId::get(),
            Balance(fixed!(2)),
        ),
        (
            LIQUIDITY_PROVIDER_C,
            PoolTokenBId::get(),
            Balance(fixed!(20)),
        ),
    ])
    .build();
    ext.execute_with(|| {
        let tech_account_id = GetPswapDistributionAccountId::get();

        for i in 0u64..5 {
            PswapDistrModule::incentive_distribution_routine(i);
        }

        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_A))
            .expect("Claiming is not expected to fail.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_B))
            .expect("Claiming is not expected to fail.");
        PswapDistrModule::claim_incentive(Origin::signed(LIQUIDITY_PROVIDER_C))
            .expect("Claiming is not expected to fail.");

        let balance_a = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_A);
        let balance_b = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_B);
        let balance_c = Tokens::free_balance(GetIncentiveAssetId::get(), &LIQUIDITY_PROVIDER_C);
        let balance_d = Tokens::free_balance(GetIncentiveAssetId::get(), &tech_account_id);
        assert_eq!(balance_a, Balance::from(11u128)); // 10 from A, 2 from B
        assert_eq!(balance_b, Balance::from(1u128)); // 1 from B
        assert_eq!(balance_c, Balance::from(12u128)); // 10 from A, 2 from B
        assert_eq!(balance_d, Balance::from(0u128));
    })
}
