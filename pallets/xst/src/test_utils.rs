use core::str::FromStr;

use crate::mock::*;
use common::{self, Oracle, SymbolName};

pub fn relay_symbol(symbol_name: &str, rate: u64) -> SymbolName {
    let euro = SymbolName::from_str(symbol_name).expect("Failed to parse `EURO` as a symbol name");
    let alice = alice();

    OracleProxy::enable_oracle(RuntimeOrigin::root(), Oracle::BandChainFeed)
        .expect("Failed to enable `Band` oracle");
    Band::add_relayers(RuntimeOrigin::root(), vec![alice.clone()]).expect("Failed to add relayers");

    // precision in band::relay is 10^9
    Band::relay(
        RuntimeOrigin::signed(alice.clone()),
        vec![(euro.clone(), rate)],
        0,
        0,
    )
    .expect("Failed to relay");
    euro
}
