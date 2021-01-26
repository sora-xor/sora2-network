use framenode_runtime::{
    bonding_curve_pool, eth_bridge, opaque::SessionKeys, AccountId, AssetSymbol, AssetsConfig,
    BabeConfig, BalancesConfig, BondingCurvePoolConfig, BridgeMultisigConfig, DEXAPIConfig,
    DEXManagerConfig, EthBridgeConfig, FarmingConfig, FaucetConfig, GenesisConfig, GetBaseAssetId,
    GrandpaConfig, IrohaMigrationConfig, LiquiditySourceType, PermissionsConfig,
    PswapDistributionConfig, PswapId, Runtime, SessionConfig, Signature, StakerStatus,
    StakingConfig, SudoConfig, SystemConfig, TechAccountId, TechnicalConfig, TokensConfig, UsdId,
    ValId, XorId, WASM_BINARY,
};

use common::prelude::{DEXInfo, FixedWrapper};
use common::{balance::Balance, fixed, hash, DEXId, Fixed, TechPurpose, PSWAP, VAL, XOR};
use frame_support::sp_runtime::Percent;
use framenode_runtime::bonding_curve_pool::{DistributionAccountData, DistributionAccounts};
use framenode_runtime::eth_bridge::AssetKind;
use grandpa::AuthorityId as GrandpaId;
use hex_literal::hex;
use permissions::Scope;
use sc_network::config::MultiaddrWithPeerId;
use sc_service::{ChainType, Properties};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{
    sp_std::iter::once,
    traits::{IdentifyAccount, Verify},
    Perbill,
};
use std::str::FromStr;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;
type Technical = technical::Module<Runtime>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Babe authority key.
pub fn authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, AuraId, BabeId, GrandpaId) {
    (
        get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
        get_account_id_from_seed::<sr25519::Public>(seed),
        get_from_seed::<AuraId>(seed),
        get_from_seed::<BabeId>(seed),
        get_from_seed::<GrandpaId>(seed),
    )
}

pub fn authority_keys_from_public_keys(
    stash_address: [u8; 32],
    controller_address: [u8; 32],
    sr25519_key: [u8; 32],
    ed25519_key: [u8; 32],
) -> (AccountId, AccountId, AuraId, BabeId, GrandpaId) {
    (
        stash_address.into(),
        controller_address.into(),
        AuraId::from_slice(&sr25519_key),
        BabeId::from_slice(&sr25519_key),
        GrandpaId::from_slice(&ed25519_key),
    )
}

fn session_keys(grandpa: GrandpaId, babe: BabeId) -> SessionKeys {
    SessionKeys { babe, grandpa }
}

pub fn dev_net() -> ChainSpec {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "XOR".into());
    properties.insert("tokenDecimals".into(), 18.into());
    ChainSpec::from_genesis(
        "SORA-dev Testnet",
        "sora-substrate-dev",
        ChainType::Live,
        move || {
            testnet_genesis(
                hex!("92c4ff71ae7492a1e6fef5d80546ea16307c560ac1063ffaa5e0e084df1e2b7e").into(),
                vec![
                    authority_keys_from_public_keys(
                        hex!("349b061381fe1e47b5dd18061f7c7f76801b41dc9c6afe0b2c4c65e0171c8b35"),
                        hex!("9c3c8836f6def559a11751c18541b9a2c81bcf9bd6ac28d978b1adfacc354456"),
                        hex!("9c3c8836f6def559a11751c18541b9a2c81bcf9bd6ac28d978b1adfacc354456"),
                        hex!("0ced48eb19e0e2809a769c35a64264c3dd39f3aa0ff132aa7caaa6730ad31f57"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("5e7df6d78fb252ecfe5e2c516a145671b9c64ee7b733a3c128af27d76e2fe74c"),
                        hex!("02bbb81a8132f9eb78ac1f2a9606055e58540f220fa1075bb3ba3d30add09e3f"),
                        hex!("02bbb81a8132f9eb78ac1f2a9606055e58540f220fa1075bb3ba3d30add09e3f"),
                        hex!("c75a2ed4012a61cf05ec6eecc4b83faedcf6a781111cc61f8e9a23ad2810bb5e"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("baa98b9fde4fc1c983998798536a63ab70b3c365ce3870dd84a230cb19093004"),
                        hex!("0ea8eafc441aa319aeaa23a74ed588f0ccd17eb3b41d12a1d8283b5f79c7b15d"),
                        hex!("0ea8eafc441aa319aeaa23a74ed588f0ccd17eb3b41d12a1d8283b5f79c7b15d"),
                        hex!("4be870c72a1ac412a5c239d701b5dd62a9e030899943faad55b48eb2c7c9dc2a"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("4eb0f6225cef84a0285a54916625846e50d86526bdece448894af0ac87792956"),
                        hex!("18b2c456464825673c63aa7866ee479b52d1a7a4bab7999408bd3568d5a02b64"),
                        hex!("18b2c456464825673c63aa7866ee479b52d1a7a4bab7999408bd3568d5a02b64"),
                        hex!("8061f3a75ef96a0d840d84cec5d42bcad43f882efdcf93b30a60c7bac6c894c1"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("22a886a8f0a0ddd031518a2bc567585b0046d02d7aacbdb058857b42da40444b"),
                        hex!("3a41a438f76d6a68b17fbd34e8a8195e5e2f74419db3bf7d914627803409ce35"),
                        hex!("3a41a438f76d6a68b17fbd34e8a8195e5e2f74419db3bf7d914627803409ce35"),
                        hex!("86320cd87cbe2881cdf3515d3a72d833099d61b4c38266437366e3b143f8835b"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("20a0225a3cafe2d5e9813025e3f1a2d9a3e50f44528ecc3bed01c13466e33316"),
                        hex!("c25eb643fd3a981a223046f32d1977644a17bb856a228d755868c1bb89d95b3d"),
                        hex!("c25eb643fd3a981a223046f32d1977644a17bb856a228d755868c1bb89d95b3d"),
                        hex!("15c652e559703197d10997d04df0081918314b77b8475d74002adaca0f3b634d"),
                    ),
                ],
                vec![
                    hex!("349b061381fe1e47b5dd18061f7c7f76801b41dc9c6afe0b2c4c65e0171c8b35").into(),
                    hex!("9c3c8836f6def559a11751c18541b9a2c81bcf9bd6ac28d978b1adfacc354456").into(),
                    hex!("5e7df6d78fb252ecfe5e2c516a145671b9c64ee7b733a3c128af27d76e2fe74c").into(),
                    hex!("02bbb81a8132f9eb78ac1f2a9606055e58540f220fa1075bb3ba3d30add09e3f").into(),
                    hex!("baa98b9fde4fc1c983998798536a63ab70b3c365ce3870dd84a230cb19093004").into(),
                    hex!("0ea8eafc441aa319aeaa23a74ed588f0ccd17eb3b41d12a1d8283b5f79c7b15d").into(),
                    hex!("4eb0f6225cef84a0285a54916625846e50d86526bdece448894af0ac87792956").into(),
                    hex!("18b2c456464825673c63aa7866ee479b52d1a7a4bab7999408bd3568d5a02b64").into(),
                    hex!("22a886a8f0a0ddd031518a2bc567585b0046d02d7aacbdb058857b42da40444b").into(),
                    hex!("3a41a438f76d6a68b17fbd34e8a8195e5e2f74419db3bf7d914627803409ce35").into(),
                    hex!("20a0225a3cafe2d5e9813025e3f1a2d9a3e50f44528ecc3bed01c13466e33316").into(),
                    hex!("c25eb643fd3a981a223046f32d1977644a17bb856a228d755868c1bb89d95b3d").into(),
                ],
                vec![
                    hex!("da96bc5065020df6d5ccc9659ae3007ddc04a6fd7f52cabe76e87b6219026b65").into(),
                    hex!("f57efdde92d350999cb41d1f2b21255d9ba7ae70cf03538ddee42a38f48a5436").into(),
                    hex!("aa79aa80b94b1cfba69c4a7d60eeb7b469e6411d1f686cc61de8adc8b1b76a69").into(),
                    hex!("60dc5adadc262770cbe904e3f65a26a89d46b70447640cd7968b49ddf5a459bc").into(),
                    hex!("70d61e980602e09ac8b5fb50658ebd345774e73b8248d3b61862ba1a9a035082").into(),
                    hex!("05918034f4a7f7c5d99cd0382aa6574ec2aba148aa3d769e50e0ac7663e36d58").into(),
                ],
                hex!("da723e9d76bd60da0ec846895c5e0ecf795b50ae652c012f27e56293277ef372").into(),
                hex!("16fec57d383a1875ab4e9786aea7a626e721a491c828f475ae63ef098f98f373").into(),
                hex!("da723e9d76bd60da0ec846895c5e0ecf795b50ae652c012f27e56293277ef372").into(),
            )
        },
        vec![],
        None,
        Some("sora-substrate-1"),
        Some(properties),
        None,
    )
}

/// # Parameters
/// * `test` - indicates if the chain spec is to be used in test environment
pub fn staging_net(test: bool) -> ChainSpec {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "XOR".into());
    properties.insert("tokenDecimals".into(), 18.into());
    let (name, id, boot_nodes) = if test {
        (
            "SORA-test",
            "sora-substrate-test",
            vec![
                MultiaddrWithPeerId::from_str("/dns/s1.tst.sora2.soramitsu.co.jp/tcp/30333/p2p/12D3KooWSG3eJ9LXNyhzUzkzqjhT3Jv35vak9zLTHTsoTiqU4mxW").unwrap(),
                MultiaddrWithPeerId::from_str("/dns/s1.tst.sora2.soramitsu.co.jp/tcp/30334/p2p/12D3KooWCfkMa5ATWfa8Edn3Lx71tfZwTU8X532Qx8jbtBnyvXyD").unwrap(),
                MultiaddrWithPeerId::from_str("/dns/s2.tst.sora2.soramitsu.co.jp/tcp/31333/p2p/12D3KooWCKC4hDHz8AxnacYg7CmeDPJL8MuJxGYHUBFZ4BjZYcCy").unwrap(),
                MultiaddrWithPeerId::from_str("/dns/s2.tst.sora2.soramitsu.co.jp/tcp/31334/p2p/12D3KooWRo4T2RxgLs1ej61g788kbYR3obU4fHu4GEfeQNEPGD2Y").unwrap(),
            ]
        )
    } else {
        (
            "SORA-staging Testnet",
            "sora-substrate-staging",
            vec![
                MultiaddrWithPeerId::from_str("/dns/s1.stg1.sora2.soramitsu.co.jp/tcp/30333/p2p/12D3KooWQf9AXopgwHsfKCweXtuePnWKieythwNa7AFwNfyemcjX").unwrap(),
                MultiaddrWithPeerId::from_str("/dns/s1.stg1.sora2.soramitsu.co.jp/tcp/30334/p2p/12D3KooWGXhnvgvUwbU831p19sy2gEdPbusN1B8P8ShuKi4JfLDH").unwrap(),
                MultiaddrWithPeerId::from_str("/dns/s2.stg1.sora2.soramitsu.co.jp/tcp/31333/p2p/12D3KooWBwZmMTKQ37dEKAR3oxcuH9YFpzUdGRTbQcKgXLEmyhob").unwrap(),
                MultiaddrWithPeerId::from_str("/dns/s2.stg1.sora2.soramitsu.co.jp/tcp/31334/p2p/12D3KooWExRdWV2CAF8oEyMYiXc9NABu8mmYLdXLtTNjjt1WjqAC").unwrap(),
            ]
        )
    };
    ChainSpec::from_genesis(
        name,
        id,
        ChainType::Live,
        move || {
            testnet_genesis(
                hex!("2c5f3fd607721d5dd9fdf26d69cdcb9294df96a8ff956b1323d69282502aaa2e").into(),
                vec![
                    authority_keys_from_public_keys(
                        hex!("dce47ff231d43281e03dd21e5890db128176d9ee20e65da331d8ae0b64863779"),
                        hex!("5683cf2ddb87bfed4f4f10ceefd44a61c0eda4fe7c63bd046cb5b3673c41c66b"),
                        hex!("5683cf2ddb87bfed4f4f10ceefd44a61c0eda4fe7c63bd046cb5b3673c41c66b"),
                        hex!("51d7f9c7f9da7a72a78f50470e56e39b7923339988506060d94f6c2e9c516be8"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("2a57402736d2b5ada9ee900e506a84436556470de7abd382031e1d90b182bd48"),
                        hex!("9a014ecc9f8d87b0315a21d2e3be84409c2fbbd9b5236910660aaa6d5e1ac05e"),
                        hex!("9a014ecc9f8d87b0315a21d2e3be84409c2fbbd9b5236910660aaa6d5e1ac05e"),
                        hex!("f0c30bbb51dd66d2111e534cd47ac553a3a342d60c4d4f44b5566c9ad26e3346"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("e493667f399170b28f3b2db4b9f28dbbabbc5da5fc21114e076768fc3c539002"),
                        hex!("8c9a6f997970057925bbc022bee892c7da318f29bbdc9d4645b6c159534d3a67"),
                        hex!("8c9a6f997970057925bbc022bee892c7da318f29bbdc9d4645b6c159534d3a67"),
                        hex!("b2e80730dd52182b324b6dfe1f0731f0f449ee2b7e257fb575f56c72a9f5af6d"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("00e8f3ad6566b446834f5361d0ed98aca3ab0c59848372f87546897345f9456f"),
                        hex!("1e7ef2261dee2d6fc8ac829e943d547bddacf4371a22555e63d4dbaf1c2e827a"),
                        hex!("1e7ef2261dee2d6fc8ac829e943d547bddacf4371a22555e63d4dbaf1c2e827a"),
                        hex!("04bd6c3c7a8f116a7a4d5578f5c1cc6e61e72d75bd7eac3333e5a300e5c17d9b"),
                    ),
                ],
                vec![
                    hex!("dce47ff231d43281e03dd21e5890db128176d9ee20e65da331d8ae0b64863779").into(),
                    hex!("5683cf2ddb87bfed4f4f10ceefd44a61c0eda4fe7c63bd046cb5b3673c41c66b").into(),
                    hex!("2a57402736d2b5ada9ee900e506a84436556470de7abd382031e1d90b182bd48").into(),
                    hex!("9a014ecc9f8d87b0315a21d2e3be84409c2fbbd9b5236910660aaa6d5e1ac05e").into(),
                    hex!("e493667f399170b28f3b2db4b9f28dbbabbc5da5fc21114e076768fc3c539002").into(),
                    hex!("8c9a6f997970057925bbc022bee892c7da318f29bbdc9d4645b6c159534d3a67").into(),
                    hex!("00e8f3ad6566b446834f5361d0ed98aca3ab0c59848372f87546897345f9456f").into(),
                    hex!("1e7ef2261dee2d6fc8ac829e943d547bddacf4371a22555e63d4dbaf1c2e827a").into(),
                ],
                vec![
                    hex!("9cbca76054814f05364abf691f9166b1be176d9b399d94dc2d88b6c4bc2b0589").into(),
                    hex!("3b2e166bca8913d9b88d7a8acdfc54c3fe92c15e347deda6a13c191c6e0cc19c").into(),
                    hex!("07f5670d08b8f3bd493ff829482a489d94494fd50dd506957e44e9fdc2e98684").into(),
                    hex!("211bb96e9f746183c05a1d583bccf513f9d8f679d6f36ecbd06609615a55b1cc").into(),
                ],
                hex!("da723e9d76bd60da0ec846895c5e0ecf795b50ae652c012f27e56293277ef372").into(),
                hex!("16fec57d383a1875ab4e9786aea7a626e721a491c828f475ae63ef098f98f373").into(),
                hex!("da723e9d76bd60da0ec846895c5e0ecf795b50ae652c012f27e56293277ef372").into(),
            )
        },
        boot_nodes,
        None,
        Some("sora-substrate-1"),
        Some(properties),
        None,
    )
}

fn bonding_curve_distribution_accounts(
) -> DistributionAccounts<DistributionAccountData<<Runtime as technical::Trait>::TechAccountId>> {
    use common::{fixed_wrapper, prelude::fixnum::ops::Numeric};
    let val_holders_coefficient = fixed_wrapper!(0.5);
    let val_holders_xor_alloc_coeff = fixed_wrapper!(0.9) * val_holders_coefficient.clone();
    let val_holders_buy_back_coefficient =
        val_holders_coefficient.clone() * (fixed_wrapper!(1) - fixed_wrapper!(0.9));
    let projects_coefficient = fixed_wrapper!(1) - val_holders_coefficient;
    let projects_sora_citizens_coeff = projects_coefficient.clone() * fixed_wrapper!(0.01);
    let projects_stores_and_shops_coeff = projects_coefficient.clone() * fixed_wrapper!(0.04);
    let projects_parliament_and_development_coeff =
        projects_coefficient.clone() * fixed_wrapper!(0.05);
    let projects_other_coeff = projects_coefficient.clone() * fixed_wrapper!(0.9);

    debug_assert_eq!(
        Fixed::ONE,
        FixedWrapper::get(
            val_holders_xor_alloc_coeff.clone()
                + projects_sora_citizens_coeff.clone()
                + projects_stores_and_shops_coeff.clone()
                + projects_parliament_and_development_coeff.clone()
                + projects_other_coeff.clone()
                + val_holders_buy_back_coefficient.clone()
        )
        .unwrap()
    );

    let xor_allocation = DistributionAccountData::new(
        TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"xor_allocation".to_vec()),
        ),
        val_holders_xor_alloc_coeff.get().unwrap(),
    );
    let sora_citizens = DistributionAccountData::new(
        TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"sora_citizens".to_vec()),
        ),
        projects_sora_citizens_coeff.get().unwrap(),
    );
    let stores_and_shops = DistributionAccountData::new(
        TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"stores_and_shops".to_vec()),
        ),
        projects_stores_and_shops_coeff.get().unwrap(),
    );
    let parliament_and_development = DistributionAccountData::new(
        TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"parliament_and_development".to_vec()),
        ),
        projects_parliament_and_development_coeff.get().unwrap(),
    );
    let projects = DistributionAccountData::new(
        TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"projects".to_vec()),
        ),
        projects_other_coeff.get().unwrap(),
    );
    let val_holders = DistributionAccountData::new(
        TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"val_holders".to_vec()),
        ),
        val_holders_buy_back_coefficient.get().unwrap(),
    );
    DistributionAccounts::<_> {
        xor_allocation,
        sora_citizens,
        stores_and_shops,
        parliament_and_development,
        projects,
        val_holders,
    }
}

pub fn local_testnet_config() -> ChainSpec {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "XOR".into());
    properties.insert("tokenDecimals".into(), 18.into());
    ChainSpec::from_genesis(
        "SORA-local Testnet",
        "sora-substrate-local",
        ChainType::Local,
        move || {
            testnet_genesis(
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                vec![
                    authority_keys_from_seed("Alice"),
                    authority_keys_from_seed("Bob"),
                    authority_keys_from_seed("Charlie"),
                    authority_keys_from_seed("Dave"),
                    /*
                    authority_keys_from_seed("Eve"),
                    authority_keys_from_seed("Ferdie"),
                    */
                ],
                vec![
                    get_account_id_from_seed::<sr25519::Public>("Alice"),
                    get_account_id_from_seed::<sr25519::Public>("Bob"),
                    get_account_id_from_seed::<sr25519::Public>("Charlie"),
                    get_account_id_from_seed::<sr25519::Public>("Dave"),
                    get_account_id_from_seed::<sr25519::Public>("Eve"),
                    get_account_id_from_seed::<sr25519::Public>("Ferdie"),
                    get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
                ],
                vec![
                    hex!("da96bc5065020df6d5ccc9659ae3007ddc04a6fd7f52cabe76e87b6219026b65").into(),
                    hex!("f57efdde92d350999cb41d1f2b21255d9ba7ae70cf03538ddee42a38f48a5436").into(),
                    hex!("aa79aa80b94b1cfba69c4a7d60eeb7b469e6411d1f686cc61de8adc8b1b76a69").into(),
                    hex!("60dc5adadc262770cbe904e3f65a26a89d46b70447640cd7968b49ddf5a459bc").into(),
                    hex!("70d61e980602e09ac8b5fb50658ebd345774e73b8248d3b61862ba1a9a035082").into(),
                    hex!("05918034f4a7f7c5d99cd0382aa6574ec2aba148aa3d769e50e0ac7663e36d58").into(),
                ],
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                get_account_id_from_seed::<sr25519::Public>("Alice"),
            )
        },
        vec![],
        None,
        None,
        Some(properties),
        None,
    )
}

fn testnet_genesis(
    root_key: AccountId,
    initial_authorities: Vec<(AccountId, AccountId, AuraId, BabeId, GrandpaId)>,
    endowed_accounts: Vec<AccountId>,
    initial_bridge_peers: Vec<AccountId>,
    dex_root: AccountId,
    tech_permissions_owner: AccountId,
    initial_assets_owner: AccountId,
) -> GenesisConfig {
    let initial_balance = 1u128 << 60;
    let initial_staking: Balance = (initial_balance / 2).into();
    let xor_fee_tech_account_id = TechAccountId::Generic(
        xor_fee::TECH_ACCOUNT_PREFIX.to_vec(),
        xor_fee::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let xor_fee_account_id: AccountId =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&xor_fee_tech_account_id)
            .expect("Failed to decode account Id");
    let faucet_tech_account_id = TechAccountId::Generic(
        faucet::TECH_ACCOUNT_PREFIX.to_vec(),
        faucet::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let faucet_account_id: AccountId =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&faucet_tech_account_id)
            .expect("Failed to decode account id");
    let initial_eth_bridge_xor_amount = 350_000_u32;
    let initial_eth_bridge_val_amount = 33_900_000_u32;
    let eth_bridge_tech_account_id = TechAccountId::Generic(
        eth_bridge::TECH_ACCOUNT_PREFIX.to_vec(),
        eth_bridge::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let eth_bridge_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&eth_bridge_tech_account_id)
            .unwrap();
    let eth_bridge_authority_tech_account_id = TechAccountId::Generic(
        eth_bridge::TECH_ACCOUNT_PREFIX.to_vec(),
        eth_bridge::TECH_ACCOUNT_AUTHORITY.to_vec(),
    );
    let eth_bridge_authority_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(
            &eth_bridge_authority_tech_account_id,
        )
        .unwrap();

    let bonding_curve_reserves_tech_account_id = TechAccountId::Generic(
        bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
        bonding_curve_pool::TECH_ACCOUNT_RESERVES.to_vec(),
    );

    let pswap_distribution_tech_account_id =
        framenode_runtime::GetPswapDistributionTechAccountId::get();
    let pswap_distribution_account_id = framenode_runtime::GetPswapDistributionAccountId::get();

    let liquidity_proxy_tech_account_id = framenode_runtime::GetLiquidityProxyTechAccountId::get();
    let liquidity_proxy_account_id = framenode_runtime::GetLiquidityProxyAccountId::get();

    let mut tech_accounts = vec![
        (xor_fee_account_id.clone(), xor_fee_tech_account_id),
        (faucet_account_id.clone(), faucet_tech_account_id.clone()),
        (
            eth_bridge_account_id.clone(),
            eth_bridge_tech_account_id.clone(),
        ),
        (
            eth_bridge_authority_account_id.clone(),
            eth_bridge_authority_tech_account_id.clone(),
        ),
        (
            pswap_distribution_account_id.clone(),
            pswap_distribution_tech_account_id.clone(),
        ),
        (
            liquidity_proxy_account_id.clone(),
            liquidity_proxy_tech_account_id.clone(),
        ),
    ];
    let accounts = bonding_curve_distribution_accounts();
    tech_accounts.push((
        Technical::tech_account_id_to_account_id(&accounts.val_holders.account_id).unwrap(),
        accounts.val_holders.account_id.clone(),
    ));
    for tech_account in &accounts.xor_distribution_accounts_as_array() {
        tech_accounts.push((
            Technical::tech_account_id_to_account_id(&tech_account).unwrap(),
            (*tech_account).to_owned(),
        ));
    }

    let iroha_migration_tech_account_id = TechAccountId::Generic(
        iroha_migration::TECH_ACCOUNT_PREFIX.to_vec(),
        iroha_migration::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let iroha_migration_account_id = technical::Module::<Runtime>::tech_account_id_to_account_id(
        &iroha_migration_tech_account_id,
    )
    .unwrap();

    GenesisConfig {
        frame_system: Some(SystemConfig {
            code: WASM_BINARY.unwrap().to_vec(),
            changes_trie_config: Default::default(),
        }),
        pallet_sudo: Some(SudoConfig {
            key: root_key.clone(),
        }),
        technical: Some(TechnicalConfig {
            account_ids_to_tech_account_ids: tech_accounts,
        }),
        pallet_babe: Some(BabeConfig {
            authorities: vec![],
        }),
        pallet_grandpa: Some(GrandpaConfig {
            authorities: vec![],
        }),
        pallet_session: Some(SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|(account, _, _, babe_id, grandpa_id)| {
                    (
                        account.clone(),
                        account.clone(),
                        session_keys(grandpa_id.clone(), babe_id.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        }),
        pallet_staking: Some(StakingConfig {
            validator_count: initial_authorities.len() as u32 * 2,
            minimum_validator_count: 1,
            stakers: initial_authorities
                .iter()
                .map(|(stash_account, account, _, _, _)| {
                    (
                        stash_account.clone(),
                        account.clone(),
                        initial_staking,
                        StakerStatus::Validator,
                    )
                })
                .collect(),
            invulnerables: initial_authorities
                .iter()
                .map(|(stash_account, _, _, _, _)| stash_account.clone())
                .collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            ..Default::default()
        }),
        assets: Some(AssetsConfig {
            endowed_assets: vec![
                (
                    XorId::get(),
                    initial_assets_owner.clone(),
                    AssetSymbol(b"XOR".to_vec()),
                    18,
                ),
                (
                    UsdId::get(),
                    initial_assets_owner.clone(),
                    AssetSymbol(b"USDT".to_vec()),
                    18,
                ),
                (
                    ValId::get(),
                    initial_assets_owner.clone(),
                    AssetSymbol(b"VAL".to_vec()),
                    18,
                ),
                (
                    PswapId::get(),
                    initial_assets_owner.clone(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    18,
                ),
            ],
        }),
        permissions: Some(PermissionsConfig {
            initial_permission_owners: vec![
                (
                    permissions::TRANSFER,
                    Scope::Unlimited,
                    vec![tech_permissions_owner.clone()],
                ),
                (
                    permissions::INIT_DEX,
                    Scope::Unlimited,
                    vec![tech_permissions_owner.clone()],
                ),
                (
                    permissions::MANAGE_DEX,
                    Scope::Limited(hash(&0u32)),
                    vec![tech_permissions_owner.clone()],
                ),
                (
                    permissions::MINT,
                    Scope::Unlimited,
                    vec![tech_permissions_owner.clone()],
                ),
                (
                    permissions::BURN,
                    Scope::Unlimited,
                    vec![tech_permissions_owner.clone()],
                ),
            ],
            initial_permissions: vec![
                (
                    dex_root.clone(),
                    Scope::Unlimited,
                    vec![permissions::INIT_DEX],
                ),
                (
                    dex_root.clone(),
                    Scope::Limited(hash(&0u32)),
                    vec![permissions::MANAGE_DEX],
                ),
                (
                    xor_fee_account_id,
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
                (
                    iroha_migration_account_id.clone(),
                    Scope::Limited(hash(&VAL)),
                    vec![permissions::MINT],
                ),
                (
                    initial_assets_owner,
                    Scope::Unlimited,
                    vec![
                        permissions::MINT,
                        permissions::BURN,
                        permissions::CREATE_FARM,
                        permissions::LOCK_TO_FARM,
                        permissions::UNLOCK_FROM_FARM,
                        permissions::CLAIM_FROM_FARM,
                    ],
                ),
                (
                    endowed_accounts[1].clone(),
                    Scope::Unlimited,
                    vec![
                        permissions::MINT,
                        permissions::BURN,
                        permissions::CREATE_FARM,
                        permissions::LOCK_TO_FARM,
                        permissions::UNLOCK_FROM_FARM,
                        permissions::CLAIM_FROM_FARM,
                    ],
                ),
                (
                    endowed_accounts[2].clone(),
                    Scope::Unlimited,
                    vec![
                        permissions::MINT,
                        permissions::BURN,
                        permissions::CREATE_FARM,
                        permissions::LOCK_TO_FARM,
                        permissions::UNLOCK_FROM_FARM,
                        permissions::CLAIM_FROM_FARM,
                    ],
                ),
                (
                    pswap_distribution_account_id,
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
            ],
        }),
        pallet_balances: Some(BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .chain(vec![root_key, faucet_account_id.clone()].into_iter())
                .map(|k| (k, initial_balance.into()))
                .chain(once((
                    eth_bridge_account_id.clone(),
                    initial_eth_bridge_xor_amount.into(),
                )))
                .collect(),
        }),
        dex_manager: Some(DEXManagerConfig {
            dex_list: vec![(
                0,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    is_public: true,
                },
            )],
        }),
        mock_liquidity_source_Instance1: None,
        mock_liquidity_source_Instance2: None,
        mock_liquidity_source_Instance3: None,
        mock_liquidity_source_Instance4: None,
        faucet: Some(FaucetConfig {
            reserves_account_id: faucet_tech_account_id,
        }),
        tokens: Some(TokensConfig {
            endowed_accounts: vec![
                (
                    faucet_account_id.clone(),
                    ValId::get(),
                    initial_balance.into(),
                ),
                (faucet_account_id, PswapId::get(), initial_balance.into()),
                (
                    eth_bridge_account_id.clone(),
                    VAL,
                    initial_eth_bridge_val_amount.into(),
                ),
            ],
        }),
        dex_api: Some(DEXAPIConfig {
            source_types: [LiquiditySourceType::XYKPool].into(),
        }),
        eth_bridge: Some(EthBridgeConfig {
            peers: initial_bridge_peers.iter().cloned().collect(),
            bridge_account: eth_bridge_account_id.clone(),
            authority_account: eth_bridge_authority_account_id.clone(),
            tokens: vec![
                (
                    XOR.into(),
                    Some(sp_core::H160::from(hex!(
                        "40fd72257597aa14c7231a7b1aaa29fce868f677"
                    ))),
                    AssetKind::SidechainOwned,
                ),
                (
                    VAL.into(),
                    Some(sp_core::H160::from(hex!(
                        "3f9feac97e5feb15d8bf98042a9a01b515da3dfb"
                    ))),
                    AssetKind::SidechainOwned,
                ),
            ],
            pswap_owners: vec![],
        }),
        bridge_multisig: Some(BridgeMultisigConfig {
            accounts: once((
                eth_bridge_account_id.clone(),
                bridge_multisig::MultisigAccount::new(
                    initial_bridge_peers,
                    Percent::from_parts(67),
                ),
            ))
            .collect(),
        }),
        bonding_curve_pool: Some(BondingCurvePoolConfig {
            distribution_accounts: accounts,
            reserves_account_id: bonding_curve_reserves_tech_account_id,
        }),
        farming: Some(FarmingConfig {
            initial_farm: (dex_root, XOR, PSWAP),
        }),
        pswap_distribution: Some(PswapDistributionConfig {
            subscribed_accounts: Vec::new(),
            burn_info: (fixed!(0), fixed!(0.000369738339021615), fixed!(0.65), 14400),
        }),
        iroha_migration: Some(IrohaMigrationConfig {
            iroha_accounts: vec![],
            account_id: iroha_migration_account_id,
        }),
    }
}
