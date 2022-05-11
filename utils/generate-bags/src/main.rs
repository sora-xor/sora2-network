use clap::Parser;
use framenode_runtime::Runtime;
use generate_bags::generate_thresholds;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Opt {
    /// How many bags to generate.
    #[clap(long, default_value = "200")]
    n_bags: usize,

    /// Where to write the output.
    output: PathBuf,

    /// The total issuance of the native currency.
    #[clap(short, long)]
    total_issuance: u128,

    /// The minimum account balance (i.e. existential deposit) for the native currency.
    #[clap(short, long)]
    minimum_balance: u128,
}

fn main() -> Result<(), std::io::Error> {
    let Opt {
        n_bags,
        output,
        total_issuance,
        minimum_balance,
    } = Opt::parse();

    crate::generate_thresholds::<Runtime>(n_bags, &output, total_issuance, minimum_balance)
}
