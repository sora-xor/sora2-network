#![allow(non_snake_case)]

use std::str::FromStr;
use std::{fs, iter};

use fixnum::{
    ops::{CheckedAdd, RoundMode, RoundingDiv, RoundingMul},
    typenum::U18,
};
use reqwest::blocking::Client;
use serde::Deserialize;

type FixedPoint = fixnum::FixedPoint<i128, U18>;

fn to_fp<'a>(s: &'a str) -> FixedPoint {
    let s = if let Some(index) = s.find(".") {
        let right = (index + 1 + 18).min(s.len());
        &s[..right]
    } else {
        s
    };
    FixedPoint::from_str(s).unwrap()
}

#[derive(Debug, Deserialize)]
struct Pair {
    reserve0: String,
    totalSupply: String,
}

#[derive(Debug, Deserialize)]
struct Snapshot {
    liquidityTokenBalance: String,
    pair: Pair,
}

#[derive(Debug, Deserialize)]
struct Snapshots {
    liquidityPositionSnapshots: Vec<Snapshot>,
}

#[derive(Debug, Deserialize)]
struct Response {
    data: Snapshots,
}

fn parse_addresses() -> Vec<String> {
    let file = fs::read_to_string("../report_full").unwrap();
    file.lines()
        .filter_map(|line| {
            if line.contains("\"address\" : \"") {
                Some(
                    line.split("\"address\" : \"")
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .nth(0)
                        .unwrap()
                        .to_string(),
                )
            } else {
                None
            }
        })
        .collect()
}

fn query<'a, 'b, 'c>(
    uri: &'b str,
    address: &'a str,
    pair: &'c str,
    data: &mut Vec<(&'a str, &'b str, &'c str, FixedPoint)>,
    invalid_queries: &mut Vec<(&'a str, &'b str, &'c str)>,
) -> Option<FixedPoint> {
    let query = r#"{ "query" : "query { liquidityPositionSnapshots(where: { user: \"$user\", pair: \"$pair\", block_lt: 12225000 }, orderBy: block ) { liquidityTokenBalance, token0PriceUSD, token1PriceUSD, pair { token0Price, token1Price, totalSupply, reserve0, reserve1 } } }" }"#;
    let query = query.replace("$user", &address).replace("$pair", pair);
    let response = if let Ok(response) = Client::new().post(uri).body(query).send() {
        response
    } else {
        invalid_queries.push((address, uri, pair));
        return None;
    };
    let response: Response = if let Ok(response) = response.json() {
        response
    } else {
        invalid_queries.push((address, uri, pair));
        return None;
    };
    if response.data.liquidityPositionSnapshots.is_empty() {
        return None;
    }
    let snapshot = if let Some(snapshot) = response.data.liquidityPositionSnapshots.last() {
        snapshot
    } else {
        invalid_queries.push((address, uri, pair));
        return None;
    };
    let balance = to_fp(&snapshot.liquidityTokenBalance);
    let total_supply = to_fp(&snapshot.pair.totalSupply);
    let reserve_0 = to_fp(&snapshot.pair.reserve0);
    // tokens = liquidityTokenBalance / pair.totalSupply * pair.reserve0
    let xor = balance
        .rdiv(total_supply, RoundMode::Floor)
        .unwrap()
        .rmul(reserve_0, RoundMode::Floor)
        .unwrap();
    if *xor.as_bits() != 0 {
        data.push((address, uri, pair, xor.clone()));
    }
    Some(xor)
}

fn main() {
    const UNISWAP_URI: &'static str = "https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v2";
    const UNISWAP_PAIRS: [&'static str; 2] = [
        "0x01962144d41415cca072900fe87bbe2992a99f10",
        "0x4fd3f9811224bf5a87bbaf002a345560c2d98d76",
    ];
    const MOONISWAP_URI: &'static str = "https://api.thegraph.com/subgraphs/name/krboktv/mooniswap";
    const MOONISWAP_PAIRS: [&'static str; 2] = [
        "0xb90d8c0c2ace705fad8ad7e447dcf3e858c20448",
        "0x215470102a05b02a3a2898f317b5382f380afc0e",
    ];
    let addresses = parse_addresses();
    let mut data = Vec::new();
    let mut totals = Vec::new();
    let mut invalid_queries = Vec::new();
    for address in &addresses {
        let mut total = fixnum::fixnum_const!(0, 18);
        for (uri, pair) in iter::repeat(UNISWAP_URI)
            .zip(UNISWAP_PAIRS.iter())
            .chain(iter::repeat(MOONISWAP_URI).zip(MOONISWAP_PAIRS.iter()))
        {
            if let Some(xor) = query(uri, address, pair, &mut data, &mut invalid_queries) {
                total = total.cadd(xor).unwrap();
            }
        }
        if *total.as_bits() != 0 {
            println!("address: {}, total: {}", address, total);
            totals.push((address, total));
        }
    }
    println!("totals: {:?}", totals);
    println!("data: {:?}", data);
    println!("invalid_queries: {:?}", invalid_queries);
}
