// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Tips:
// * not(feature = "private-net") means "main net", however, given that "main net" is the default option and Cargo doesn't provide any way to disable "main net" if any "private net" is specified, we have to rely on such constructions.

#![allow(unused_imports, unused_macros, dead_code)]

use framenode_runtime::GenesisConfig;

use common::prelude::{Balance, DEXInfo, FixedWrapper};
use common::{
    balance, fixed, hash, DEXId, Fixed, TechPurpose, DAI, DEFAULT_BALANCE_PRECISION, ETH, PSWAP,
    USDT, VAL, XOR,
};
use frame_support::sp_runtime::Percent;
use framenode_runtime::eth_bridge::{AssetConfig, NetworkConfig};
use framenode_runtime::multicollateral_bonding_curve_pool::{
    DistributionAccount, DistributionAccountData, DistributionAccounts,
};
use framenode_runtime::opaque::SessionKeys;
use framenode_runtime::{
    eth_bridge, AccountId, AssetId, AssetName, AssetSymbol, AssetsConfig, BabeConfig,
    BalancesConfig, BridgeMultisigConfig, CouncilConfig, DEXAPIConfig, DEXManagerConfig,
    DemocracyConfig, EthBridgeConfig, GetBaseAssetId, GetParliamentAccountId, GetPswapAssetId,
    GetValAssetId, GetXorAssetId, GrandpaConfig, ImOnlineId, IrohaMigrationConfig,
    LiquiditySourceType, MulticollateralBondingCurvePoolConfig, PermissionsConfig,
    PswapDistributionConfig, RewardsConfig, Runtime, SessionConfig, StakerStatus, StakingConfig,
    SystemConfig, TechAccountId, TechnicalConfig, TokensConfig, TradingPairConfig, WASM_BINARY,
};
use hex_literal::hex;
use permissions::Scope;
use sc_finality_grandpa::AuthorityId as GrandpaId;
use sc_network::config::MultiaddrWithPeerId;
use sc_service::{ChainType, Properties};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{Public, H160};
use sp_runtime::sp_std::iter::once;
use sp_runtime::traits::Zero;
use sp_runtime::Perbill;
use std::str::FromStr;

#[cfg(feature = "private-net")]
use framenode_runtime::{FaucetConfig, SudoConfig};
use framenode_runtime::{Signature, TechnicalCommitteeConfig};
use sp_core::{sr25519, Pair};
use sp_runtime::traits::{IdentifyAccount, Verify};

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;
type Technical = technical::Module<Runtime>;
type AccountPublic = <Signature as Verify>::Signer;

// The macro is used in rewards_*.in.
// It's required instead of vec! because vec! places all data on the stack and it causes overflow.
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

macro_rules! our_include {
    ($x:expr) => {{
        #[cfg(feature = "include-real-files")]
        let output = include!($x);

        #[cfg(not(feature = "include-real-files"))]
        let output = Default::default();

        output
    }};
}

macro_rules! our_include_bytes {
    ($x:expr) => {{
        #[cfg(feature = "include-real-files")]
        static OUTPUT: &'static [u8] = include_bytes!($x);

        #[cfg(not(feature = "include-real-files"))]
        static OUTPUT: &'static [u8] = &[];

        OUTPUT
    }};
}

/// Helper function to generate a crypto pair from seed
fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

/// Helper function to generate an account ID from seed
fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Babe authority key.
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

fn session_keys(grandpa: GrandpaId, babe: BabeId, im_online: ImOnlineId) -> SessionKeys {
    SessionKeys {
        babe,
        grandpa,
        im_online,
    }
}

struct EthBridgeParams {
    xor_master_contract_address: H160,
    xor_contract_address: H160,
    val_master_contract_address: H160,
    val_contract_address: H160,
    bridge_contract_address: H160,
}

fn calculate_reserves(accounts: &Vec<(H160, Balance)>) -> Balance {
    accounts.iter().fold(0, |sum, (_, balance)| sum + balance)
}

// dev uses code
// #[cfg(all(feature = "dev-net", not(feature = "coded-nets")))]
// pub fn dev_net() -> Result<ChainSpec, String> {
//     ChainSpec::from_json_bytes(&our_include_bytes!("./bytes/chain_spec_dev.json")[..])
// }

pub fn staging_net() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(&our_include_bytes!("./bytes/chain_spec_staging.json")[..])
}

pub fn test_net() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(&our_include_bytes!("./bytes/chain_spec_test.json")[..])
}

// Main net is not ready yet.
// It still uses staging nodes.
// #[cfg(all(not(feature = "private-net"), not(feature = "coded-nets")))]
// pub fn main_net() -> Result<ChainSpec, String> {
//     ChainSpec::from_json_bytes(&our_include_bytes!("./bytes/chain_spec_main.json")[..])
// }

#[cfg(feature = "private-net")]
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
                true,
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
                    hex!("a63e5398515c405aba87c13b56d344f1a7d32d2226062fac396d58154d45380a").into(),
                    hex!("62f53d93e5ab9b26ccb7b9625abfe76a3d5fb3b732c039f3322bfe3f35503401").into(),
                    hex!("c84c2c4395322b7935bf9eba08a392e5c485b0a984b5c38c8174a89c6b24750c").into(),
                    hex!("8af75f561b714320205491d7571cf6d3df650143e2862b36c7b823d1de0bd244").into(),
                    hex!("a492d53531934d57acc5c2a852a724272b0a0d6571cc5b0e2433bebbb334e13c").into(),
                    hex!("5c6e091530ae1891eb33a9abc24727239b84bf8e458306b7cd4740662343b84c").into(),
                    hex!("7653840f435e7412fbaf0eb6331206b325de62e036435458a16155c43393f504").into(),
                    hex!("e813415062749d4bbea338d8a69b9cc5be02af0fdf8c96ba2d50733aaf32cb50").into(),
                    hex!("e08d567d824152adcf53b8dca949756be895b6b8bebb5f9fa55959e9473e0c7f").into(),
                ],
                vec![
                    hex!("da96bc5065020df6d5ccc9659ae3007ddc04a6fd7f52cabe76e87b6219026b65").into(),
                    hex!("f57efdde92d350999cb41d1f2b21255d9ba7ae70cf03538ddee42a38f48a5436").into(),
                    hex!("aa79aa80b94b1cfba69c4a7d60eeb7b469e6411d1f686cc61de8adc8b1b76a69").into(),
                    hex!("60dc5adadc262770cbe904e3f65a26a89d46b70447640cd7968b49ddf5a459bc").into(),
                    hex!("70d61e980602e09ac8b5fb50658ebd345774e73b8248d3b61862ba1a9a035082").into(),
                    hex!("05918034f4a7f7c5d99cd0382aa6574ec2aba148aa3d769e50e0ac7663e36d58").into(),
                ],
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
                vec![
                    hex!("a63e5398515c405aba87c13b56d344f1a7d32d2226062fac396d58154d45380a").into(),
                    hex!("62f53d93e5ab9b26ccb7b9625abfe76a3d5fb3b732c039f3322bfe3f35503401").into(),
                    hex!("c84c2c4395322b7935bf9eba08a392e5c485b0a984b5c38c8174a89c6b24750c").into(),
                    hex!("8af75f561b714320205491d7571cf6d3df650143e2862b36c7b823d1de0bd244").into(),
                    hex!("a492d53531934d57acc5c2a852a724272b0a0d6571cc5b0e2433bebbb334e13c").into(),
                    hex!("5c6e091530ae1891eb33a9abc24727239b84bf8e458306b7cd4740662343b84c").into(),
                ],
                vec![
                    hex!("7653840f435e7412fbaf0eb6331206b325de62e036435458a16155c43393f504").into(),
                    hex!("e813415062749d4bbea338d8a69b9cc5be02af0fdf8c96ba2d50733aaf32cb50").into(),
                    hex!("e08d567d824152adcf53b8dca949756be895b6b8bebb5f9fa55959e9473e0c7f").into(),
                ],
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
#[cfg(feature = "private-net")]
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
                false,
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
                vec![],
                vec![
                    hex!("9cbca76054814f05364abf691f9166b1be176d9b399d94dc2d88b6c4bc2b0589").into(),
                    hex!("3b2e166bca8913d9b88d7a8acdfc54c3fe92c15e347deda6a13c191c6e0cc19c").into(),
                    hex!("07f5670d08b8f3bd493ff829482a489d94494fd50dd506957e44e9fdc2e98684").into(),
                    hex!("211bb96e9f746183c05a1d583bccf513f9d8f679d6f36ecbd06609615a55b1cc").into(),
                ],
                eth_bridge_params,
                vec![],
                vec![],
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
) -> DistributionAccounts<DistributionAccountData<DistributionAccount<AccountId, TechAccountId>>> {
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
        DistributionAccount::TechAccount(TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"xor_allocation".to_vec()),
        )),
        val_holders_xor_alloc_coeff.get().unwrap(),
    );
    let sora_citizens = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"sora_citizens".to_vec()),
        )),
        projects_sora_citizens_coeff.get().unwrap(),
    );
    let stores_and_shops = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"stores_and_shops".to_vec()),
        )),
        projects_stores_and_shops_coeff.get().unwrap(),
    );
    let parliament_and_development = DistributionAccountData::new(
        DistributionAccount::Account(GetParliamentAccountId::get()),
        projects_parliament_and_development_coeff.get().unwrap(),
    );
    let projects = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"projects".to_vec()),
        )),
        projects_other_coeff.get().unwrap(),
    );
    let val_holders = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            DEXId::Polkaswap.into(),
            TechPurpose::Identifier(b"val_holders".to_vec()),
        )),
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

#[cfg(feature = "private-net")]
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
                false,
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
                    hex!("7edf2a2d157cc835131581bc068b7172a00af1a10008049f05a2308737912633").into(),
                    hex!("aa7c410fe2d9a0b96ba392c4cef95d3bf8761047297747e9118ee6d1df9f6558").into(),
                    hex!("30e87994d26e4123d585d5d8c46116bbc196a6f5a4ed87a3ee24a2dbada9a66d").into(),
                    hex!("30fbd05409cf5f6a8ae6afaa05e9861405d8fa710d0b4c8d088f155cb0b87749").into(),
                    hex!("20c706cba79f03fc2ed233da544a3e75a81dcae43b0a4edf72719307fd21cb1b").into(),
                    hex!("8297172611ad3b085258d518f849a5533271d760f729669c9f8863971d70c372").into(),
                    hex!("4a2fe11a37dfb548c64def2cbd8d5332bbd56571627b91b81c82970ceb7eec2b").into(),
                    hex!("903a885138c4a187f13383fdb08b8e6b308c7021fdab12dc20e3aef9870e1146").into(),
                    hex!("d0d773018d19aab81052c4d038783ecfee77fb4b5fdc266b5a25568c0102640b").into(),
                ],
                vec![
                    hex!("7edf2a2d157cc835131581bc068b7172a00af1a10008049f05a2308737912633").into(),
                    hex!("aa7c410fe2d9a0b96ba392c4cef95d3bf8761047297747e9118ee6d1df9f6558").into(),
                    hex!("30e87994d26e4123d585d5d8c46116bbc196a6f5a4ed87a3ee24a2dbada9a66d").into(),
                    hex!("30fbd05409cf5f6a8ae6afaa05e9861405d8fa710d0b4c8d088f155cb0b87749").into(),
                    hex!("20c706cba79f03fc2ed233da544a3e75a81dcae43b0a4edf72719307fd21cb1b").into(),
                    hex!("8297172611ad3b085258d518f849a5533271d760f729669c9f8863971d70c372").into(),
                    hex!("4a2fe11a37dfb548c64def2cbd8d5332bbd56571627b91b81c82970ceb7eec2b").into(),
                    hex!("903a885138c4a187f13383fdb08b8e6b308c7021fdab12dc20e3aef9870e1146").into(),
                    hex!("d0d773018d19aab81052c4d038783ecfee77fb4b5fdc266b5a25568c0102640b").into(),
                ],
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
                vec![
                    hex!("7edf2a2d157cc835131581bc068b7172a00af1a10008049f05a2308737912633").into(),
                    hex!("aa7c410fe2d9a0b96ba392c4cef95d3bf8761047297747e9118ee6d1df9f6558").into(),
                    hex!("30e87994d26e4123d585d5d8c46116bbc196a6f5a4ed87a3ee24a2dbada9a66d").into(),
                    hex!("30fbd05409cf5f6a8ae6afaa05e9861405d8fa710d0b4c8d088f155cb0b87749").into(),
                    hex!("20c706cba79f03fc2ed233da544a3e75a81dcae43b0a4edf72719307fd21cb1b").into(),
                    hex!("8297172611ad3b085258d518f849a5533271d760f729669c9f8863971d70c372").into(),
                ],
                vec![
                    hex!("4a2fe11a37dfb548c64def2cbd8d5332bbd56571627b91b81c82970ceb7eec2b").into(),
                    hex!("903a885138c4a187f13383fdb08b8e6b308c7021fdab12dc20e3aef9870e1146").into(),
                    hex!("d0d773018d19aab81052c4d038783ecfee77fb4b5fdc266b5a25568c0102640b").into(),
                ],
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
#[cfg(feature = "private-net")]
fn testnet_genesis(
    dev: bool,
    root_key: AccountId,
    initial_authorities: Vec<(AccountId, AccountId, AuraId, BabeId, GrandpaId, ImOnlineId)>,
    endowed_accounts: Vec<AccountId>,
    initial_bridge_peers: Vec<AccountId>,
    eth_bridge_params: EthBridgeParams,
    council_accounts: Vec<AccountId>,
    technical_committee_accounts: Vec<AccountId>,
) -> GenesisConfig {
    // Initial balances
    let initial_staking = balance!(100);
    let initial_eth_bridge_xor_amount = balance!(350000);
    let initial_eth_bridge_val_amount = balance!(33900000);
    let initial_pswap_tbc_rewards = balance!(2500000000);
    let initial_pswap_market_maker_rewards = balance!(400000000);

    let parliament_investment_fund =
        hex!("048cfcacbdebe828dffa1267d830d45135cd40238286f838f5a95432a1bbf851").into();
    let parliament_investment_fund_balance = balance!(33000000);

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

    let mbc_pool_free_reserves_tech_account_id =
        framenode_runtime::GetMbcPoolFreeReservesTechAccountId::get();
    let mbc_pool_free_reserves_account_id =
        framenode_runtime::GetMbcPoolFreeReservesAccountId::get();

    let market_maker_rewards_tech_account_id =
        framenode_runtime::GetMarketMakerRewardsTechAccountId::get();
    let market_maker_rewards_account_id = framenode_runtime::GetMarketMakerRewardsAccountId::get();

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

    let dex_root_tech_account_id =
        TechAccountId::Generic(b"SYSTEM_ACCOUNT".to_vec(), b"DEX_ROOT".to_vec());
    let dex_root_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&dex_root_tech_account_id)
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
            mbc_pool_free_reserves_account_id.clone(),
            mbc_pool_free_reserves_tech_account_id.clone(),
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
        (
            market_maker_rewards_account_id.clone(),
            market_maker_rewards_tech_account_id.clone(),
        ),
    ];
    let accounts = bonding_curve_distribution_accounts();
    for account in &accounts.accounts() {
        match account {
            DistributionAccount::Account(_) => continue,
            DistributionAccount::TechAccount(account) => {
                tech_accounts.push((
                    Technical::tech_account_id_to_account_id(account).unwrap(),
                    account.to_owned(),
                ));
            }
        }
    }
    let mut balances = vec![
        (eth_bridge_account_id.clone(), initial_eth_bridge_xor_amount),
        (assets_and_permissions_account_id.clone(), 0),
        (xor_fee_account_id.clone(), 0),
        (dex_root_account_id.clone(), 0),
        (iroha_migration_account_id.clone(), 0),
        (pswap_distribution_account_id.clone(), 0),
        (mbc_reserves_account_id.clone(), 0),
        (mbc_pool_rewards_account_id.clone(), 0),
        (mbc_pool_free_reserves_account_id.clone(), 0),
        (market_maker_rewards_account_id.clone(), 0),
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
    .chain(
        endowed_accounts
            .iter()
            .cloned()
            .map(|account| (account, initial_staking)),
    )
    .collect::<Vec<_>>();

    #[cfg(not(feature = "include-real-files"))]
    let rewards_config = RewardsConfig {
        reserves_account_id: rewards_tech_account_id,
        val_owners: vec![
            (
                hex!("d170A274320333243b9F860e8891C6792DE1eC19").into(),
                balance!(995),
            ),
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

    #[cfg(feature = "include-real-files")]
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
        (
            parliament_investment_fund,
            VAL,
            parliament_investment_fund_balance,
        ),
        (
            market_maker_rewards_account_id.clone(),
            PSWAP,
            initial_pswap_market_maker_rewards,
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

    let iroha_migration_config = IrohaMigrationConfig {
        iroha_accounts: if dev {
            our_include!("bytes/iroha_migration_accounts_dev.in")
        } else {
            our_include!("bytes/iroha_migration_accounts_staging.in")
        },
        account_id: iroha_migration_account_id.clone(),
    };
    let initial_collateral_assets = vec![DAI.into(), VAL.into(), PSWAP.into(), ETH.into()];
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
                (
                    ETH.into(),
                    eth_bridge_account_id.clone(),
                    AssetSymbol(b"ETH".to_vec()),
                    AssetName(b"Ether".to_vec()),
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
                    dex_root_account_id.clone(),
                    Scope::Limited(hash(&0u32)),
                    vec![permissions::MANAGE_DEX],
                ),
                (
                    dex_root_account_id.clone(),
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
                (
                    mbc_pool_free_reserves_account_id.clone(),
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
        trading_pair: Some(TradingPairConfig {
            trading_pairs: initial_collateral_assets
                .iter()
                .cloned()
                .map(|target_asset_id| {
                    (
                        DEXId::Polkaswap.into(),
                        common::TradingPair {
                            base_asset_id: XOR.into(),
                            target_asset_id,
                        },
                    )
                })
                .collect(),
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
                        precision: 18,
                    },
                    AssetConfig::Sidechain {
                        id: ETH.into(),
                        sidechain_id: hex!("0000000000000000000000000000000000000000").into(),
                        owned: false,
                        precision: 18,
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
            initial_collateral_assets,
            free_reserves_account_id: mbc_pool_free_reserves_account_id,
        }),
        pswap_distribution: Some(PswapDistributionConfig {
            subscribed_accounts: Vec::new(),
            burn_info: (fixed!(0.1), fixed!(0.000357), fixed!(0.65)),
        }),
        iroha_migration: Some(iroha_migration_config),
        rewards: Some(rewards_config),
        pallet_collective_Instance1: Some(CouncilConfig {
            members: council_accounts,
            phantom: Default::default(),
        }),
        pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
            members: technical_committee_accounts,
            phantom: Default::default(),
        }),
        pallet_democracy: Some(DemocracyConfig::default()),
        pallet_elections_phragmen: Default::default(),
        pallet_membership_Instance1: Default::default(),
        pallet_im_online: Default::default(),
    }
}

/// # Parameters
#[cfg(not(feature = "private-net"))]
pub fn main_net_coded() -> ChainSpec {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "XOR".into());
    properties.insert("tokenDecimals".into(), 18.into());
    let name = "SORA";
    let id = "sora-substrate-main-net";
    // SORA main-net node address. We should have 2 nodes.
    let boot_nodes = vec![
              MultiaddrWithPeerId::from_str("/dns/v1.sora2.soramitsu.co.jp/tcp/30333/p2p/12D3KooWDQmg87ET849KaCjNn8ZL59pQ9giXiZDmmAvNZNNgAmLp").unwrap(), //Prod value
              MultiaddrWithPeerId::from_str("/dns/v2.sora2.soramitsu.co.jp/tcp/30334/p2p/12D3KooWHDMg3N6nZB2o8FD41vEDP8vrTfRnZrbZSf2CiH87EELW").unwrap()  //Prod value
            ];
    ChainSpec::from_genesis(
        name,
        id,
        ChainType::Live,
        move || {
            let eth_bridge_params = EthBridgeParams {
                xor_master_contract_address: hex!("c08edf13be9b9cc584c5da8004ce7e6be63c1316") //Prod value
                    .into(),
                xor_contract_address: hex!("40fd72257597aa14c7231a7b1aaa29fce868f677").into(), //Prod value
                val_master_contract_address: hex!("d1eeb2f30016fffd746233ee12c486e7ca8efef1") //Prod value
                    .into(),
                val_contract_address: hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4").into(), //Prod value
                // Bridge contract address taken from test-net
                bridge_contract_address: hex!("64fb0ca483b356832cd97958e6b23df783fb7ced").into(),
            };

            // SORA main-net node address. We should have 2 nodes.
            // Currently filled with staging example values
            mainnet_genesis(
                vec![
                    authority_keys_from_public_keys(
                        hex!("207ed7bbf6fa0685dca5f24d6773a58ab9c710512d1087db5e47e0fe0f357239"), //Prod value
                        hex!("14d500b666dbacc20535f8d2d4f039a8ace624c58e880d573980553774d7ff1a"), //Prod value
                        hex!("14d500b666dbacc20535f8d2d4f039a8ace624c58e880d573980553774d7ff1a"), //Prod value
                        hex!("71e6acfa06696ae5d962a36b88ddf4b0c7d5751a7107a2db1e6947ee2442f573"), //Prod value
                    ),
                    authority_keys_from_public_keys(
                        hex!("94ee828c3455a327dde32f577e27f0b8a4c42b3fb626ee27f0004f7cf02bd332"), //Prod value
                        hex!("38364b218e599f78f2b52f34748908addce908881b2c76296c50b2494261c004"), //Prod value
                        hex!("38364b218e599f78f2b52f34748908addce908881b2c76296c50b2494261c004"), //Prod value
                        hex!("d603aea460c53393cfd2e2eb2820bb138738288502488fd6431fa93f7b59642d"), //Prod value
                    ),
                ],
                vec![
                    hex!("a3bcbf3044069ac13c30d662a204d8368c266e2f0e8cf603c7bfb2b7b5daae55").into(), //Prod value
                    hex!("297c03e65c2930daa7c6067a2bb853819b61ed49b70de2f3219a2eb6ec0364aa").into(), //Prod value
                ],
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

#[cfg(not(feature = "private-net"))]
fn mainnet_genesis(
    initial_authorities: Vec<(AccountId, AccountId, AuraId, BabeId, GrandpaId, ImOnlineId)>,
    initial_bridge_peers: Vec<AccountId>,
    eth_bridge_params: EthBridgeParams,
) -> GenesisConfig {
    // Minimum stake for an active validator
    let initial_staking = balance!(1);
    // XOR amount which already exists on Ethereum
    let initial_eth_bridge_xor_amount = balance!(350000);
    // VAL amount which already exists on SORA_1 and Ethereum. Partially can be migrated directly from SORA_1. Not yet decided finally.
    let initial_eth_bridge_val_amount = balance!(33900000);
    // Initial token bonding curve PSWAP rewards according to 10 bln PSWAP total supply.
    let initial_pswap_tbc_rewards = balance!(2500000000);
    // Initial market maker PSWAP rewards.
    let initial_pswap_market_maker_rewards = balance!(400000000);

    let parliament_investment_fund =
        hex!("048cfcacbdebe828dffa1267d830d45135cd40238286f838f5a95432a1bbf851").into();
    let parliament_investment_fund_balance = balance!(33000000);

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

    let mbc_pool_free_reserves_tech_account_id =
        framenode_runtime::GetMbcPoolFreeReservesTechAccountId::get();
    let mbc_pool_free_reserves_account_id =
        framenode_runtime::GetMbcPoolFreeReservesAccountId::get();

    let market_maker_rewards_tech_account_id =
        framenode_runtime::GetMarketMakerRewardsTechAccountId::get();
    let market_maker_rewards_account_id = framenode_runtime::GetMarketMakerRewardsAccountId::get();

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

    let dex_root_tech_account_id =
        TechAccountId::Generic(b"SYSTEM_ACCOUNT".to_vec(), b"DEX_ROOT".to_vec());
    let dex_root_account_id =
        technical::Module::<Runtime>::tech_account_id_to_account_id(&dex_root_tech_account_id)
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
            mbc_pool_free_reserves_account_id.clone(),
            mbc_pool_free_reserves_tech_account_id.clone(),
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
        (
            market_maker_rewards_account_id.clone(),
            market_maker_rewards_tech_account_id.clone(),
        ),
    ];
    let accounts = bonding_curve_distribution_accounts();
    for account in &accounts.accounts() {
        match account {
            DistributionAccount::Account(_) => continue,
            DistributionAccount::TechAccount(account) => {
                tech_accounts.push((
                    Technical::tech_account_id_to_account_id(account).unwrap(),
                    account.to_owned(),
                ));
            }
        }
    }
    let rewards_config = RewardsConfig {
        reserves_account_id: rewards_tech_account_id,
        val_owners: our_include!("bytes/rewards_val_owners.in"),
        pswap_farm_owners: our_include!("bytes/rewards_pswap_farm_owners.in"),
        pswap_waifu_owners: our_include!("bytes/rewards_pswap_waifu_owners.in"),
    };
    let initial_collateral_assets = vec![DAI.into(), VAL.into(), PSWAP.into(), ETH.into()];

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
                (
                    DAI.into(),
                    eth_bridge_account_id.clone(),
                    AssetSymbol(b"DAI".to_vec()),
                    AssetName(b"Dai Stablecoin".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    ETH.into(),
                    eth_bridge_account_id.clone(),
                    AssetSymbol(b"ETH".to_vec()),
                    AssetName(b"Ether".to_vec()),
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
                    dex_root_account_id.clone(),
                    Scope::Limited(hash(&0u32)),
                    vec![permissions::MANAGE_DEX],
                ),
                (
                    dex_root_account_id.clone(),
                    Scope::Unlimited,
                    vec![permissions::CREATE_FARM],
                ),
                (
                    xor_fee_account_id.clone(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
                (
                    iroha_migration_account_id.clone(),
                    Scope::Limited(hash(&VAL)),
                    vec![permissions::MINT],
                ),
                (
                    assets_and_permissions_account_id.clone(),
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
                    pswap_distribution_account_id.clone(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
                (
                    mbc_reserves_account_id.clone(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
                (
                    mbc_pool_free_reserves_account_id.clone(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
            ],
        }),
        pallet_balances: Some(BalancesConfig {
            balances: vec![
                (eth_bridge_account_id.clone(), initial_eth_bridge_xor_amount),
                (assets_and_permissions_account_id.clone(), 0),
                (xor_fee_account_id.clone(), 0),
                (dex_root_account_id.clone(), 0),
                (iroha_migration_account_id.clone(), 0),
                (pswap_distribution_account_id.clone(), 0),
                (mbc_reserves_account_id.clone(), 0),
                (mbc_pool_rewards_account_id.clone(), 0),
                (mbc_pool_free_reserves_account_id.clone(), 0),
                (market_maker_rewards_account_id.clone(), 0),
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
                (
                    parliament_investment_fund,
                    VAL,
                    parliament_investment_fund_balance,
                ),
                (
                    market_maker_rewards_account_id.clone(),
                    PSWAP,
                    initial_pswap_market_maker_rewards,
                ),
            ],
        }),
        trading_pair: Some(TradingPairConfig {
            trading_pairs: initial_collateral_assets
                .iter()
                .cloned()
                .map(|target_asset_id| {
                    (
                        DEXId::Polkaswap.into(),
                        common::TradingPair {
                            base_asset_id: XOR.into(),
                            target_asset_id,
                        },
                    )
                })
                .collect(),
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
                        sidechain_id: hex!("6b175474e89094c44da98b954eedeac495271d0f").into(),
                        owned: false,
                        precision: 18,
                    },
                    AssetConfig::Sidechain {
                        id: ETH.into(),
                        sidechain_id: hex!("0000000000000000000000000000000000000000").into(),
                        owned: false,
                        precision: 18,
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
            initial_collateral_assets,
            free_reserves_account_id: mbc_pool_free_reserves_account_id,
        }),
        pswap_distribution: Some(PswapDistributionConfig {
            subscribed_accounts: Vec::new(),
            burn_info: (fixed!(0.1), fixed!(0.000357), fixed!(0.65)),
        }),
        iroha_migration: Some(IrohaMigrationConfig {
            iroha_accounts: our_include!("bytes/iroha_migration_accounts.in"),
            account_id: iroha_migration_account_id,
        }),
        rewards: Some(rewards_config),
        pallet_collective_Instance1: Some(CouncilConfig::default()),
        pallet_collective_Instance2: Some(TechnicalCommitteeConfig::default()),
        pallet_democracy: Some(DemocracyConfig::default()),
        pallet_elections_phragmen: Default::default(),
        pallet_membership_Instance1: Default::default(),
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
