use core::str::FromStr;

use crate::mock::*;
use common::{self, Oracle, SymbolName};

pub fn relay_new_symbol(symbol_name: &str, rate: u64) -> SymbolName {
    let symbol =
        SymbolName::from_str(symbol_name).expect("Failed to parse `symbol_name` as a symbol name");
    let alice = alice();

    OracleProxy::enable_oracle(RuntimeOrigin::root(), Oracle::BandChainFeed)
        .expect("Failed to enable `Band` oracle");
    Band::add_relayers(RuntimeOrigin::root(), vec![alice.clone()]).expect("Failed to add relayers");

    // precision in band::relay is 10^9
    Band::relay(
        RuntimeOrigin::signed(alice.clone()),
        vec![(symbol.clone(), rate)].try_into().unwrap(),
        0,
        0,
    )
    .expect("Failed to relay");
    symbol
}

pub fn relay_symbol(symbol: SymbolName, rate: u64) {
    let alice = alice();

    Band::relay(
        RuntimeOrigin::signed(alice.clone()),
        vec![(symbol, rate)].try_into().unwrap(),
        0,
        0,
    )
    .expect("Failed to relay");
}
