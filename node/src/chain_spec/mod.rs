// Tips:
// * not(feature = "private-net") means "main net", however, given that "main net" is the default option and Cargo doesn't provide any way to disable "main net" if any "private net" is specified, we have to rely on such constructions.

use framenode_runtime::GenesisConfig;

#[cfg(all(feature = "private-net", feature = "coded-nets"))]
use common::DAI;

#[cfg(feature = "coded-nets")]
use {
    common::prelude::{Balance, DEXInfo, FixedWrapper},
    common::{
        balance, fixed, hash, DEXId, Fixed, TechPurpose, DEFAULT_BALANCE_PRECISION, PSWAP, VAL, XOR,
    },
    frame_support::sp_runtime::Percent,
    framenode_runtime::bonding_curve_pool::{DistributionAccountData, DistributionAccounts},
    framenode_runtime::eth_bridge::{AssetConfig, NetworkConfig},
    framenode_runtime::opaque::SessionKeys,
    framenode_runtime::{
        eth_bridge, AccountId, AssetName, AssetSymbol, AssetsConfig, BabeConfig, BalancesConfig,
        BridgeMultisigConfig, CouncilConfig, DEXAPIConfig, DEXManagerConfig, DemocracyConfig,
        EthBridgeConfig, FarmingConfig, GetBaseAssetId, GetParliamentTechAccountId,
        GetPswapAssetId, GetValAssetId, GetXorAssetId, GrandpaConfig, ImOnlineId,
        IrohaMigrationConfig, LiquiditySourceType, MulticollateralBondingCurvePoolConfig,
        PermissionsConfig, PswapDistributionConfig, RewardsConfig, Runtime, SessionConfig,
        StakerStatus, StakingConfig, SystemConfig, TechAccountId, TechnicalConfig, TokensConfig,
        WASM_BINARY,
    },
    hex_literal::hex,
    permissions::Scope,
    sc_finality_grandpa::AuthorityId as GrandpaId,
    sc_service::{ChainType, Properties},
    sp_consensus_aura::sr25519::AuthorityId as AuraId,
    sp_consensus_babe::AuthorityId as BabeId,
    sp_core::{Public, H160},
    sp_runtime::sp_std::iter::once,
    sp_runtime::traits::Zero,
    sp_runtime::Perbill,
};
#[cfg(all(
    any(
        feature = "stage-net",
        feature = "test-net",
        not(feature = "private-net")
    ),
    feature = "coded-nets"
))]
use {sc_network::config::MultiaddrWithPeerId, std::str::FromStr};

#[cfg(all(feature = "private-net", feature = "coded-nets"))]
use {
    framenode_runtime::{FaucetConfig, Signature, SudoConfig, TechnicalCommitteeConfig},
    sp_core::{sr25519, Pair},
    sp_runtime::traits::{IdentifyAccount, Verify},
};

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;
#[cfg(feature = "coded-nets")]
type Technical = technical::Module<Runtime>;
#[cfg(all(feature = "private-net", feature = "coded-nets"))]
type AccountPublic = <Signature as Verify>::Signer;

// The macro is used in rewards_*.in.
// It's required instead of vec! because vec! places all data on the stack and it causes overflow.
#[cfg(all(
    any(
        feature = "stage-net",
        feature = "test-net",
        not(feature = "private-net")
    ),
    feature = "coded-nets"
))]
macro_rules! vec_push {
    ($($x:expr),+ $(,)?) => (
        {
            let mut vec = Vec::new();
            $(
                vec.push($x);
            )+
            vec
        }
    );
}

/// Helper function to generate a crypto pair from seed
#[cfg(all(feature = "private-net", feature = "coded-nets"))]
fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

/// Helper function to generate an account ID from seed
#[cfg(all(feature = "private-net", feature = "coded-nets"))]
fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Babe authority key.
#[cfg(all(feature = "private-net", feature = "coded-nets"))]
pub fn authority_keys_from_seed(
    seed: &str,
) -> (AccountId, AccountId, AuraId, BabeId, GrandpaId, ImOnlineId) {
    (
        get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
        get_account_id_from_seed::<sr25519::Public>(seed),
        get_from_seed::<AuraId>(seed),
        get_from_seed::<BabeId>(seed),
        get_from_seed::<GrandpaId>(seed),
        get_from_seed::<ImOnlineId>(seed),
    )
}

#[cfg(all(
    any(
        feature = "dev-net",
        feature = "stage-net",
        feature = "test-net",
        not(feature = "private-net")
    ),
    feature = "coded-nets"
))]
pub fn authority_keys_from_public_keys(
    stash_address: [u8; 32],
    controller_address: [u8; 32],
    sr25519_key: [u8; 32],
    ed25519_key: [u8; 32],
) -> (AccountId, AccountId, AuraId, BabeId, GrandpaId, ImOnlineId) {
    (
        stash_address.into(),
        controller_address.into(),
        AuraId::from_slice(&sr25519_key),
        BabeId::from_slice(&sr25519_key),
        GrandpaId::from_slice(&ed25519_key),
        ImOnlineId::from_slice(&sr25519_key),
    )
}

#[cfg(feature = "coded-nets")]
fn session_keys(grandpa: GrandpaId, babe: BabeId, im_online: ImOnlineId) -> SessionKeys {
    SessionKeys {
        babe,
        grandpa,
        im_online,
    }
}

#[cfg(feature = "coded-nets")]
struct EthBridgeParams {
    xor_master_contract_address: H160,
    xor_contract_address: H160,
    val_master_contract_address: H160,
    val_contract_address: H160,
    bridge_contract_address: H160,
}

#[cfg(feature = "coded-nets")]
fn calculate_reserves(accounts: &Vec<(H160, Balance)>) -> Balance {
    accounts.iter().fold(0, |sum, (_, balance)| sum + balance)
}

// dev uses code
// #[cfg(all(feature = "dev-net", not(feature = "coded-nets")))]
// pub fn dev_net() -> Result<ChainSpec, String> {
//     ChainSpec::from_json_bytes(&include_bytes!("./bytes/chain_spec_dev.json")[..])
// }

#[cfg(all(feature = "stage-net", not(feature = "coded-nets")))]
pub fn staging_net() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(&include_bytes!("./bytes/chain_spec_staging.json")[..])
}

#[cfg(all(feature = "test-net", not(feature = "coded-nets")))]
pub fn test_net() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(&include_bytes!("./bytes/chain_spec_test.json")[..])
}

// Main net is not ready yet.
// It still uses staging nodes.
// #[cfg(all(not(feature = "private-net"), not(feature = "coded-nets")))]
// pub fn main_net() -> Result<ChainSpec, String> {
//     ChainSpec::from_json_bytes(&include_bytes!("./bytes/chain_spec_main.json")[..])
// }

#[cfg(all(feature = "coded-nets", feature = "dev-net"))]
pub fn dev_net_coded() -> ChainSpec {
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
                EthBridgeParams {
                    xor_master_contract_address: hex!("12c6a709925783f49fcca0b398d13b0d597e6e1c")
                        .into(),
                    xor_contract_address: hex!("02ffdae478412dbde6bbd5cda8ff05c0960e0c45").into(),
                    val_master_contract_address: hex!("47e229aa491763038f6a505b4f85d8eb463f0962")
                        .into(),
                    val_contract_address: hex!("68339de68c9af6577c54867728dbb2db9d7368bf").into(),
                    bridge_contract_address: hex!("24390c8f6cbd5d152c30226f809f4e3f153b88d4")
                        .into(),
                },
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
#[cfg(all(
    any(feature = "stage-net", feature = "test-net"),
    feature = "coded-nets"
))]
pub fn staging_net_coded(test: bool) -> ChainSpec {
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
            let eth_bridge_params = if test {
                EthBridgeParams {
                    xor_master_contract_address: hex!("3520adc7b99e55c77efd0e0d379d07d08a7488cc")
                        .into(),
                    xor_contract_address: hex!("83ba842e5e26a4eda2466891221187aabbc33692").into(),
                    val_master_contract_address: hex!("a55236ad2162a47a52316f86d688fbd71b520945")
                        .into(),
                    val_contract_address: hex!("7fcb82ab5a4762f0f18287ece64d4ec74b6071c0").into(),
                    bridge_contract_address: hex!("c3d1366ad8ffd17acc484d66aa403b490b9ef134")
                        .into(),
                }
            } else {
                EthBridgeParams {
                    xor_master_contract_address: hex!("cceb41100aa2a9a6f144d7c1f876070b810bf7ae")
                        .into(),
                    xor_contract_address: hex!("dc1c024535118f6de6d999c23fc31e33bc2cafc9").into(),
                    val_master_contract_address: hex!("d7f81ed173cb3af28f983670164df30851fba678")
                        .into(),
                    val_contract_address: hex!("725c6b8cd3621eba4e0ccc40d532e7025b925a65").into(),
                    bridge_contract_address: hex!("077c2ec37d28709ce01ae740209bfbe185bd1eaa")
                        .into(),
                }
            };
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
                    authority_keys_from_public_keys(
                        hex!("621067638b1d90bfd52450c0569b5318b283bc4eccfaaf0175adada721a86e17"),
                        hex!("f2ea7d239d82dbc64013f88ffc7837c28fcaeaf2787bc07d0b9bd89d9d672f21"),
                        hex!("f2ea7d239d82dbc64013f88ffc7837c28fcaeaf2787bc07d0b9bd89d9d672f21"),
                        hex!("c047e7799daa62017ad18264f704225a140417fe6b726e7cbb97a4c397b78b91"),
                    ),
                    authority_keys_from_public_keys(
                        hex!("664601bab694be726d919e310c3744fd5432ed125e20b46f7ebdcfe01848c72d"),
                        hex!("98a28d465f3bf349f19c27394a4f4b08fe18e5e75088733c86adb728c1797179"),
                        hex!("98a28d465f3bf349f19c27394a4f4b08fe18e5e75088733c86adb728c1797179"),
                        hex!("d4d791cf11cecc39805499e534ab8c07366f444f0efd6d73731f2e3555cbc2d9"),
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
                    hex!("621067638b1d90bfd52450c0569b5318b283bc4eccfaaf0175adada721a86e17").into(),
                    hex!("f2ea7d239d82dbc64013f88ffc7837c28fcaeaf2787bc07d0b9bd89d9d672f21").into(),
                    hex!("664601bab694be726d919e310c3744fd5432ed125e20b46f7ebdcfe01848c72d").into(),
                    hex!("98a28d465f3bf349f19c27394a4f4b08fe18e5e75088733c86adb728c1797179").into(),
                ],
                vec![
                    hex!("9cbca76054814f05364abf691f9166b1be176d9b399d94dc2d88b6c4bc2b0589").into(),
                    hex!("3b2e166bca8913d9b88d7a8acdfc54c3fe92c15e347deda6a13c191c6e0cc19c").into(),
                    hex!("07f5670d08b8f3bd493ff829482a489d94494fd50dd506957e44e9fdc2e98684").into(),
                    hex!("211bb96e9f746183c05a1d583bccf513f9d8f679d6f36ecbd06609615a55b1cc").into(),
                ],
                hex!("da723e9d76bd60da0ec846895c5e0ecf795b50ae652c012f27e56293277ef372").into(),
                eth_bridge_params,
            )
        },
        boot_nodes,
        None,
        Some("sora-substrate-1"),
        Some(properties),
        None,
    )
}

#[cfg(feature = "coded-nets")]
fn bonding_curve_distribution_accounts(
) -> DistributionAccounts<DistributionAccountData<<Runtime as technical::Config>::TechAccountId>> {
    use common::fixed_wrapper;
    use common::prelude::fixnum::ops::One;
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
        GetParliamentTechAccountId::get(),
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

#[cfg(all(feature = "private-net", feature = "coded-nets"))]
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
                EthBridgeParams {
                    xor_master_contract_address: hex!("12c6a709925783f49fcca0b398d13b0d597e6e1c")
                        .into(),
                    xor_contract_address: hex!("02ffdae478412dbde6bbd5cda8ff05c0960e0c45").into(),
                    val_master_contract_address: hex!("47e229aa491763038f6a505b4f85d8eb463f0962")
                        .into(),
                    val_contract_address: hex!("68339de68c9af6577c54867728dbb2db9d7368bf").into(),
                    bridge_contract_address: hex!("64fb0ca483b356832cd97958e6b23df783fb7ced")
                        .into(),
                },
            )
        },
        vec![],
        None,
        None,
        Some(properties),
        None,
    )
}

// Some variables are only changed if faucet is enabled
#[cfg(all(feature = "private-net", feature = "coded-nets"))]
fn testnet_genesis(
    root_key: AccountId,
    initial_authorities: Vec<(AccountId, AccountId, AuraId, BabeId, GrandpaId, ImOnlineId)>,
    endowed_accounts: Vec<AccountId>,
    initial_bridge_peers: Vec<AccountId>,
    dex_root: AccountId,
    eth_bridge_params: EthBridgeParams,
) -> GenesisConfig {
    // Initial balances
    let initial_staking = balance!(5000);
    let initial_eth_bridge_xor_amount = balance!(350000);
    let initial_eth_bridge_val_amount = balance!(33900000);
    let initial_pswap_tbc_rewards = balance!(25000000);

    // Initial accounts
    let xor_fee_tech_account_id = TechAccountId::Generic(
        xor_fee::TECH_ACCOUNT_PREFIX.to_vec(),
        xor_fee::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let xor_fee_account_id: AccountId =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&xor_fee_tech_account_id)
            .expect("Failed to decode account Id");

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

    let mbc_reserves_tech_account_id = framenode_runtime::GetMbcReservesTechAccountId::get();
    let mbc_reserves_account_id = framenode_runtime::GetMbcReservesAccountId::get();

    let pswap_distribution_tech_account_id =
        framenode_runtime::GetPswapDistributionTechAccountId::get();
    let pswap_distribution_account_id = framenode_runtime::GetPswapDistributionAccountId::get();

    let mbc_pool_rewards_tech_account_id = framenode_runtime::GetMbcPoolRewardsTechAccountId::get();
    let mbc_pool_rewards_account_id = framenode_runtime::GetMbcPoolRewardsAccountId::get();

    let liquidity_proxy_tech_account_id = framenode_runtime::GetLiquidityProxyTechAccountId::get();
    let liquidity_proxy_account_id = framenode_runtime::GetLiquidityProxyAccountId::get();

    let iroha_migration_tech_account_id = TechAccountId::Generic(
        iroha_migration::TECH_ACCOUNT_PREFIX.to_vec(),
        iroha_migration::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let iroha_migration_account_id = technical::Module::<Runtime>::tech_account_id_to_account_id(
        &iroha_migration_tech_account_id,
    )
    .unwrap();

    let rewards_tech_account_id = TechAccountId::Generic(
        rewards::TECH_ACCOUNT_PREFIX.to_vec(),
        rewards::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let rewards_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&rewards_tech_account_id)
            .unwrap();

    let assets_and_permissions_tech_account_id =
        TechAccountId::Generic(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
    let assets_and_permissions_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(
            &assets_and_permissions_tech_account_id,
        )
        .unwrap();

    let mut tech_accounts = vec![
        (xor_fee_account_id.clone(), xor_fee_tech_account_id),
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
        (
            mbc_reserves_account_id.clone(),
            mbc_reserves_tech_account_id.clone(),
        ),
        (
            mbc_pool_rewards_account_id.clone(),
            mbc_pool_rewards_tech_account_id.clone(),
        ),
        (
            iroha_migration_account_id.clone(),
            iroha_migration_tech_account_id.clone(),
        ),
        (rewards_account_id.clone(), rewards_tech_account_id.clone()),
        (
            assets_and_permissions_account_id.clone(),
            assets_and_permissions_tech_account_id.clone(),
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
    let mut balances = vec![
        (eth_bridge_account_id.clone(), initial_eth_bridge_xor_amount),
        (assets_and_permissions_account_id.clone(), 0),
        (xor_fee_account_id.clone(), 0),
        (dex_root.clone(), 0),
        (iroha_migration_account_id.clone(), 0),
        (pswap_distribution_account_id.clone(), 0),
        (mbc_reserves_account_id.clone(), 0),
    ]
    .into_iter()
    .chain(
        initial_authorities
            .iter()
            .cloned()
            .map(|(k1, ..)| (k1, initial_staking)),
    )
    .chain(
        initial_authorities
            .iter()
            .cloned()
            .map(|(_, k2, ..)| (k2, initial_staking)),
    )
    .collect::<Vec<_>>();

    #[cfg(not(any(feature = "stage-net", feature = "test-net")))]
    let rewards_config = RewardsConfig {
        reserves_account_id: rewards_tech_account_id,
        val_owners: vec![
            (
                hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into(),
                balance!(111),
            ),
            (
                hex!("D67fea281B2C5dC3271509c1b628E0867a9815D7").into(),
                balance!(444),
            ),
        ],
        pswap_farm_owners: vec![
            (
                hex!("4fE143cDD48791cB364823A41e018AEC5cBb9AbB").into(),
                balance!(222),
            ),
            (
                hex!("D67fea281B2C5dC3271509c1b628E0867a9815D7").into(),
                balance!(555),
            ),
        ],
        pswap_waifu_owners: vec![(
            hex!("886021F300dC809269CFC758A2364a2baF63af0c").into(),
            balance!(333),
        )],
    };

    #[cfg(any(feature = "stage-net", feature = "test-net"))]
    let rewards_config = RewardsConfig {
        reserves_account_id: rewards_tech_account_id,
        val_owners: include!("bytes/rewards_val_owners.in"),
        pswap_farm_owners: include!("bytes/rewards_pswap_farm_owners.in"),
        pswap_waifu_owners: include!("bytes/rewards_pswap_waifu_owners.in"),
    };

    let rewards_val_reserves = calculate_reserves(&rewards_config.val_owners);
    let rewards_pswap_reserves = calculate_reserves(&rewards_config.pswap_farm_owners)
        + calculate_reserves(&rewards_config.pswap_waifu_owners);
    let mut tokens_endowed_accounts = vec![
        (
            rewards_account_id.clone(),
            GetValAssetId::get(),
            rewards_val_reserves,
        ),
        (
            rewards_account_id,
            GetPswapAssetId::get(),
            rewards_pswap_reserves,
        ),
        (
            eth_bridge_account_id.clone(),
            VAL,
            initial_eth_bridge_val_amount,
        ),
        (
            mbc_pool_rewards_account_id.clone(),
            PSWAP,
            initial_pswap_tbc_rewards,
        ),
    ];
    let faucet_config = {
        let initial_faucet_balance = balance!(6000000000);
        let faucet_tech_account_id = TechAccountId::Generic(
            faucet::TECH_ACCOUNT_PREFIX.to_vec(),
            faucet::TECH_ACCOUNT_MAIN.to_vec(),
        );
        let faucet_account_id: AccountId =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&faucet_tech_account_id)
                .expect("Failed to decode account id");
        tech_accounts.push((faucet_account_id.clone(), faucet_tech_account_id.clone()));
        balances.push((faucet_account_id.clone(), initial_faucet_balance));
        tokens_endowed_accounts.push((faucet_account_id.clone(), VAL, initial_faucet_balance));
        tokens_endowed_accounts.push((faucet_account_id, PSWAP, initial_faucet_balance));
        FaucetConfig {
            reserves_account_id: faucet_tech_account_id,
        }
    };

    #[cfg(any(feature = "dev", feature = "stage-net", feature = "test-net"))]
    let iroha_migration_config = IrohaMigrationConfig {
        iroha_accounts: include!("bytes/iroha_migration_accounts.in"),
        account_id: iroha_migration_account_id.clone(),
    };

    #[cfg(not(any(feature = "dev", feature = "stage-net", feature = "test-net")))]
    let iroha_migration_config = IrohaMigrationConfig {
        iroha_accounts: vec![],
        account_id: iroha_migration_account_id.clone(),
    };

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
                .map(|(account, _, _, babe_id, grandpa_id, im_online_id)| {
                    (
                        account.clone(),
                        account.clone(),
                        session_keys(grandpa_id.clone(), babe_id.clone(), im_online_id.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        }),
        pallet_staking: Some(StakingConfig {
            validator_count: 69,
            minimum_validator_count: 1,
            stakers: initial_authorities
                .iter()
                .map(|(stash_account, account, _, _, _, _)| {
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
                .map(|(stash_account, _, _, _, _, _)| stash_account.clone())
                .collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            ..Default::default()
        }),
        assets: Some(AssetsConfig {
            endowed_assets: vec![
                (
                    GetXorAssetId::get(),
                    assets_and_permissions_account_id.clone(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                // (
                //     UsdId::get(),
                //     assets_and_permissions_account_id.clone(),
                //     AssetSymbol(b"USDT".to_vec()),
                //     AssetName(b"Tether USD".to_vec()),
                //     18,
                //     Balance::zero(),
                //     true,
                // ),
                (
                    GetValAssetId::get(),
                    assets_and_permissions_account_id.clone(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    GetPswapAssetId::get(),
                    assets_and_permissions_account_id.clone(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"Polkaswap".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    DAI.into(),
                    eth_bridge_account_id.clone(),
                    AssetSymbol(b"DAI".to_vec()),
                    AssetName(b"Dai Stablecoin".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
            ],
        }),
        permissions: Some(PermissionsConfig {
            initial_permission_owners: vec![
                (
                    permissions::MANAGE_DEX,
                    Scope::Limited(hash(&0u32)),
                    vec![assets_and_permissions_account_id.clone()],
                ),
                (
                    permissions::MINT,
                    Scope::Unlimited,
                    vec![assets_and_permissions_account_id.clone()],
                ),
                (
                    permissions::BURN,
                    Scope::Unlimited,
                    vec![assets_and_permissions_account_id.clone()],
                ),
            ],
            initial_permissions: vec![
                (
                    dex_root.clone(),
                    Scope::Limited(hash(&0u32)),
                    vec![permissions::MANAGE_DEX],
                ),
                (
                    dex_root.clone(),
                    Scope::Unlimited,
                    vec![permissions::CREATE_FARM],
                ),
                (
                    xor_fee_account_id,
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
                (
                    iroha_migration_account_id,
                    Scope::Limited(hash(&VAL)),
                    vec![permissions::MINT],
                ),
                (
                    assets_and_permissions_account_id,
                    Scope::Unlimited,
                    vec![
                        permissions::MINT,
                        permissions::BURN,
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
                (
                    mbc_reserves_account_id,
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
            ],
        }),
        pallet_balances: Some(BalancesConfig { balances }),
        dex_manager: Some(DEXManagerConfig {
            dex_list: vec![(
                0,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    is_public: true,
                },
            )],
        }),
        faucet: Some(faucet_config),
        tokens: Some(TokensConfig {
            endowed_accounts: tokens_endowed_accounts,
        }),
        dex_api: Some(DEXAPIConfig {
            source_types: [
                LiquiditySourceType::XYKPool,
                LiquiditySourceType::MulticollateralBondingCurvePool,
            ]
            .into(),
        }),
        eth_bridge: Some(EthBridgeConfig {
            authority_account: eth_bridge_authority_account_id.clone(),
            networks: vec![NetworkConfig {
                initial_peers: initial_bridge_peers.iter().cloned().collect(),
                bridge_account_id: eth_bridge_account_id.clone(),
                assets: vec![
                    AssetConfig::Sidechain {
                        id: XOR.into(),
                        sidechain_id: eth_bridge_params.xor_contract_address,
                        owned: true,
                        precision: DEFAULT_BALANCE_PRECISION,
                    },
                    AssetConfig::Sidechain {
                        id: VAL.into(),
                        sidechain_id: eth_bridge_params.val_contract_address,
                        owned: true,
                        precision: DEFAULT_BALANCE_PRECISION,
                    },
                    AssetConfig::Sidechain {
                        id: DAI.into(),
                        sidechain_id: hex!("5592ec0cfb4dbc12d3ab100b257153436a1f0fea").into(),
                        owned: false,
                        precision: DEFAULT_BALANCE_PRECISION,
                    },
                ],
                bridge_contract_address: eth_bridge_params.bridge_contract_address,
                reserves: vec![
                    (XOR.into(), balance!(350000)),
                    (VAL.into(), balance!(33900000)),
                ],
            }],
            xor_master_contract_address: eth_bridge_params.xor_master_contract_address,
            val_master_contract_address: eth_bridge_params.val_master_contract_address,
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
        multicollateral_bonding_curve_pool: Some(MulticollateralBondingCurvePoolConfig {
            distribution_accounts: accounts,
            reserves_account_id: mbc_reserves_tech_account_id,
            reference_asset_id: DAI.into(),
            incentives_account_id: mbc_pool_rewards_account_id,
            initial_collateral_assets: [DAI.into(), VAL.into(), PSWAP.into()].into(),
        }),
        farming: Some(FarmingConfig {
            initial_farm: (dex_root, XOR, PSWAP),
        }),
        pswap_distribution: Some(PswapDistributionConfig {
            subscribed_accounts: Vec::new(),
            burn_info: (fixed!(0.1), fixed!(0.000357), fixed!(0.65)),
        }),
        iroha_migration: Some(iroha_migration_config),
        rewards: Some(rewards_config),
        pallet_collective_Instance1: Some(CouncilConfig::default()),
        pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
            members: endowed_accounts
                .iter()
                .take((endowed_accounts.len() + 1) / 2)
                .cloned()
                .collect(),
            phantom: Default::default(),
        }),
        pallet_democracy: Some(DemocracyConfig::default()),
        pallet_im_online: Default::default(),
    }
}

/// # Parameters
#[cfg(all(feature = "coded-nets", not(feature = "private-net")))]
pub fn main_net_coded() -> ChainSpec {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "XOR".into());
    properties.insert("tokenDecimals".into(), 18.into());
    let name = "SORA";
    let id = "sora-substrate-main-net";
    //SORA main-net node address. We should have 22 node. As much as possible from Community and other from Soramitsu.
    // Currently filled with staging values
    let boot_nodes =  vec![
              MultiaddrWithPeerId::from_str("/dns/s1.stg1.sora2.soramitsu.co.jp/tcp/30333/p2p/12D3KooWQf9AXopgwHsfKCweXtuePnWKieythwNa7AFwNfyemcjX").unwrap(),
              MultiaddrWithPeerId::from_str("/dns/s1.stg1.sora2.soramitsu.co.jp/tcp/30334/p2p/12D3KooWGXhnvgvUwbU831p19sy2gEdPbusN1B8P8ShuKi4JfLDH").unwrap()
            ];
    ChainSpec::from_genesis(
        name,
        id,
        ChainType::Live,
        move || {
            let eth_bridge_params = EthBridgeParams {
                xor_master_contract_address: hex!("c08edf13be9b9cc584c5da8004ce7e6be63c1316")
                    .into(),
                xor_contract_address: hex!("40fd72257597aa14c7231a7b1aaa29fce868f677").into(),
                val_master_contract_address: hex!("d1eeb2f30016fffd746233ee12c486e7ca8efef1")
                    .into(),
                val_contract_address: hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4").into(),
                // Bridge contract address taken from test-net
                bridge_contract_address: hex!("64fb0ca483b356832cd97958e6b23df783fb7ced").into(),
            };

            //SORA main-net node address. We should have 22 node. As much as possible from Community and other from Soramitsu.
            // Currently filled with staging example values
            mainnet_genesis(
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
                ],
                vec![
                    hex!("dce47ff231d43281e03dd21e5890db128176d9ee20e65da331d8ae0b64863779").into(),
                    hex!("5683cf2ddb87bfed4f4f10ceefd44a61c0eda4fe7c63bd046cb5b3673c41c66b").into(),
                    hex!("2a57402736d2b5ada9ee900e506a84436556470de7abd382031e1d90b182bd48").into(),
                    hex!("9a014ecc9f8d87b0315a21d2e3be84409c2fbbd9b5236910660aaa6d5e1ac05e").into(),
                ],
                vec![
                    hex!("9cbca76054814f05364abf691f9166b1be176d9b399d94dc2d88b6c4bc2b0589").into(),
                    hex!("3b2e166bca8913d9b88d7a8acdfc54c3fe92c15e347deda6a13c191c6e0cc19c").into(),
                ],
                hex!("da723e9d76bd60da0ec846895c5e0ecf795b50ae652c012f27e56293277ef372").into(),
                eth_bridge_params,
            )
        },
        boot_nodes,
        None,
        Some("sora-substrate-1"),
        Some(properties),
        None,
    )
}

#[cfg(all(feature = "coded-nets", not(feature = "private-net")))]
fn mainnet_genesis(
    initial_authorities: Vec<(AccountId, AccountId, AuraId, BabeId, GrandpaId, ImOnlineId)>,
    _endowed_accounts: Vec<AccountId>,
    initial_bridge_peers: Vec<AccountId>,
    dex_root: AccountId,
    eth_bridge_params: EthBridgeParams,
) -> GenesisConfig {
    // Minimum stake for an active validator
    let initial_staking = balance!(5000);
    // XOR amount which already exists on Ethereum
    let initial_eth_bridge_xor_amount = balance!(350000);
    // VAL amount which already exists on SORA_1 and Ethereum. Partially can be migrated directly from SORA_1. Not yet decided finally.
    let initial_eth_bridge_val_amount = balance!(33900000);
    // Initial token bonding curve PSWAP rewards
    let initial_pswap_tbc_rewards = balance!(25000000);

    // Initial accounts
    let xor_fee_tech_account_id = TechAccountId::Generic(
        xor_fee::TECH_ACCOUNT_PREFIX.to_vec(),
        xor_fee::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let xor_fee_account_id: AccountId =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&xor_fee_tech_account_id)
            .expect("Failed to decode account Id");

    // Bridge peers multisignature account
    let eth_bridge_tech_account_id = TechAccountId::Generic(
        eth_bridge::TECH_ACCOUNT_PREFIX.to_vec(),
        eth_bridge::TECH_ACCOUNT_MAIN.to_vec(),
    );
    // Wrapping of bridge peers multisignature account
    let eth_bridge_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&eth_bridge_tech_account_id)
            .unwrap();
    // Bridge authority account expected to be managed by voting
    let eth_bridge_authority_tech_account_id = TechAccountId::Generic(
        eth_bridge::TECH_ACCOUNT_PREFIX.to_vec(),
        eth_bridge::TECH_ACCOUNT_AUTHORITY.to_vec(),
    );
    // Wrapper for Bridge authority account expected to be managed by voting
    let eth_bridge_authority_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(
            &eth_bridge_authority_tech_account_id,
        )
        .unwrap();

    let mbc_reserves_tech_account_id = framenode_runtime::GetMbcReservesTechAccountId::get();
    let mbc_reserves_account_id = framenode_runtime::GetMbcReservesAccountId::get();

    let pswap_distribution_tech_account_id =
        framenode_runtime::GetPswapDistributionTechAccountId::get();
    let pswap_distribution_account_id = framenode_runtime::GetPswapDistributionAccountId::get();

    let mbc_pool_rewards_tech_account_id = framenode_runtime::GetMbcPoolRewardsTechAccountId::get();
    let mbc_pool_rewards_account_id = framenode_runtime::GetMbcPoolRewardsAccountId::get();

    let liquidity_proxy_tech_account_id = framenode_runtime::GetLiquidityProxyTechAccountId::get();
    let liquidity_proxy_account_id = framenode_runtime::GetLiquidityProxyAccountId::get();

    let iroha_migration_tech_account_id = TechAccountId::Generic(
        iroha_migration::TECH_ACCOUNT_PREFIX.to_vec(),
        iroha_migration::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let iroha_migration_account_id = technical::Module::<Runtime>::tech_account_id_to_account_id(
        &iroha_migration_tech_account_id,
    )
    .unwrap();

    let rewards_tech_account_id = TechAccountId::Generic(
        rewards::TECH_ACCOUNT_PREFIX.to_vec(),
        rewards::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let rewards_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&rewards_tech_account_id)
            .unwrap();

    let assets_and_permissions_tech_account_id =
        TechAccountId::Generic(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
    let assets_and_permissions_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(
            &assets_and_permissions_tech_account_id,
        )
        .unwrap();

    let mut tech_accounts = vec![
        (xor_fee_account_id.clone(), xor_fee_tech_account_id),
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
        (
            mbc_reserves_account_id.clone(),
            mbc_reserves_tech_account_id.clone(),
        ),
        (
            mbc_pool_rewards_account_id.clone(),
            mbc_pool_rewards_tech_account_id.clone(),
        ),
        (
            iroha_migration_account_id.clone(),
            iroha_migration_tech_account_id.clone(),
        ),
        (rewards_account_id.clone(), rewards_tech_account_id.clone()),
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
    let rewards_config = RewardsConfig {
        reserves_account_id: rewards_tech_account_id,
        val_owners: include!("bytes/rewards_val_owners.in"),
        pswap_farm_owners: include!("bytes/rewards_pswap_farm_owners.in"),
        pswap_waifu_owners: include!("bytes/rewards_pswap_waifu_owners.in"),
    };
    GenesisConfig {
        frame_system: Some(SystemConfig {
            code: WASM_BINARY.unwrap().to_vec(),
            changes_trie_config: Default::default(),
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
                .map(|(account, _, _, babe_id, grandpa_id, im_online_id)| {
                    (
                        account.clone(),
                        account.clone(),
                        session_keys(grandpa_id.clone(), babe_id.clone(), im_online_id.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        }),
        pallet_staking: Some(StakingConfig {
            validator_count: 69,
            minimum_validator_count: 1,
            stakers: initial_authorities
                .iter()
                .map(|(stash_account, account, _, _, _, _)| {
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
                .map(|(stash_account, _, _, _, _, _)| stash_account.clone())
                .collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            ..Default::default()
        }),
        assets: Some(AssetsConfig {
            endowed_assets: vec![
                (
                    GetXorAssetId::get(),
                    assets_and_permissions_account_id.clone(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    GetValAssetId::get(),
                    assets_and_permissions_account_id.clone(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    GetPswapAssetId::get(),
                    assets_and_permissions_account_id.clone(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"Polkaswap".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
            ],
        }),
        permissions: Some(PermissionsConfig {
            initial_permission_owners: vec![
                (
                    permissions::MANAGE_DEX,
                    Scope::Limited(hash(&0u32)),
                    vec![assets_and_permissions_account_id.clone()],
                ),
                (
                    permissions::MINT,
                    Scope::Unlimited,
                    vec![assets_and_permissions_account_id.clone()],
                ),
                (
                    permissions::BURN,
                    Scope::Unlimited,
                    vec![assets_and_permissions_account_id.clone()],
                ),
            ],
            initial_permissions: vec![
                (
                    dex_root.clone(),
                    Scope::Limited(hash(&0u32)),
                    vec![permissions::MANAGE_DEX],
                ),
                (
                    dex_root.clone(),
                    Scope::Unlimited,
                    vec![permissions::CREATE_FARM],
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
                    assets_and_permissions_account_id,
                    Scope::Unlimited,
                    vec![
                        permissions::MINT,
                        permissions::BURN,
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
                (
                    mbc_reserves_account_id,
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
            ],
        }),
        pallet_balances: Some(BalancesConfig {
            balances: vec![(eth_bridge_account_id.clone(), initial_eth_bridge_xor_amount)]
                .into_iter()
                .chain(
                    initial_authorities
                        .iter()
                        .cloned()
                        .map(|(k1, ..)| (k1, initial_staking)),
                )
                .chain(
                    initial_authorities
                        .iter()
                        .cloned()
                        .map(|(_, k2, ..)| (k2, initial_staking)),
                )
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
        tokens: Some(TokensConfig {
            endowed_accounts: vec![
                (
                    rewards_account_id.clone(),
                    GetValAssetId::get(),
                    calculate_reserves(&rewards_config.val_owners),
                ),
                (
                    rewards_account_id,
                    GetPswapAssetId::get(),
                    calculate_reserves(&rewards_config.pswap_farm_owners)
                        + calculate_reserves(&rewards_config.pswap_waifu_owners),
                ),
                (
                    eth_bridge_account_id.clone(),
                    VAL,
                    initial_eth_bridge_val_amount,
                ),
                (
                    mbc_pool_rewards_account_id.clone(),
                    PSWAP,
                    initial_pswap_tbc_rewards,
                ),
            ],
        }),
        dex_api: Some(DEXAPIConfig {
            source_types: [
                LiquiditySourceType::XYKPool,
                LiquiditySourceType::MulticollateralBondingCurvePool,
            ]
            .into(),
        }),
        eth_bridge: Some(EthBridgeConfig {
            authority_account: eth_bridge_authority_account_id.clone(),
            networks: vec![NetworkConfig {
                initial_peers: initial_bridge_peers.iter().cloned().collect(),
                bridge_account_id: eth_bridge_account_id.clone(),
                assets: vec![
                    AssetConfig::Sidechain {
                        id: XOR.into(),
                        sidechain_id: eth_bridge_params.xor_contract_address,
                        owned: true,
                        precision: DEFAULT_BALANCE_PRECISION,
                    },
                    AssetConfig::Sidechain {
                        id: VAL.into(),
                        sidechain_id: eth_bridge_params.val_contract_address,
                        owned: true,
                        precision: DEFAULT_BALANCE_PRECISION,
                    },
                ],
                bridge_contract_address: eth_bridge_params.bridge_contract_address,
                reserves: vec![
                    (XOR.into(), balance!(350000)),
                    (VAL.into(), balance!(33900000)),
                ],
            }],
            xor_master_contract_address: eth_bridge_params.xor_master_contract_address,
            val_master_contract_address: eth_bridge_params.val_master_contract_address,
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
        multicollateral_bonding_curve_pool: Some(MulticollateralBondingCurvePoolConfig {
            distribution_accounts: accounts,
            reserves_account_id: mbc_reserves_tech_account_id,
            reference_asset_id: Default::default(),
            incentives_account_id: mbc_pool_rewards_account_id,
            initial_collateral_assets: Vec::new(),
        }),
        farming: Some(FarmingConfig {
            initial_farm: (dex_root, XOR, PSWAP),
        }),
        pswap_distribution: Some(PswapDistributionConfig {
            subscribed_accounts: Vec::new(),
            burn_info: (fixed!(0.1), fixed!(0.000357), fixed!(0.65)),
        }),
        iroha_migration: Some(IrohaMigrationConfig {
            iroha_accounts: include!("bytes/iroha_migration_accounts.in"),
            account_id: iroha_migration_account_id,
        }),
        rewards: Some(rewards_config),
        pallet_collective_Instance1: Some(CouncilConfig::default()),
        pallet_collective_Instance2: Default::default(),
        pallet_democracy: Some(DemocracyConfig::default()),
        pallet_im_online: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use common::balance;

    #[test]
    fn calculate_reserves() {
        let accounts = vec![
            (
                hex!("3520adc7b99e55c77efd0e0d379d07d08a7488cc").into(),
                balance!(100),
            ),
            (
                hex!("3520adc7b99e55c77efd0e0d379d07d08a7488cc").into(),
                balance!(23.4000000),
            ),
            (
                hex!("3520adc7b99e55c77efd0e0d379d07d08a7488cc").into(),
                balance!(0.05678),
            ),
        ];
        assert_eq!(super::calculate_reserves(&accounts), balance!(123.45678));
    }
}
