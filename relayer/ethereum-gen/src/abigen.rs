use ethers::contract::Abigen;
use ethers::solc::Solc;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    args.next().unwrap(); // skip program name

    let contract_name = args.next().unwrap_or("SimpleStorage".to_owned());
    let contract: String = args.next().unwrap_or("examples/contract.sol".to_owned());

    println!("Generating bindings for {}: {}\n", contract_name, contract);

    // compile it
    let abi = if contract.ends_with(".sol") {
        let contracts = Solc::default().compile_source(&contract)?;
        let abi = contracts
            .get(&contract, &contract_name)
            .unwrap()
            .abi
            .unwrap();
        serde_json::to_string(abi).unwrap()
    } else {
        String::from_utf8(std::fs::read(contract)?)?
    };

    println!("ABI");

    let bindings = Abigen::new(&contract_name, abi)
        .map_err(|e| {
            println!("Abigen error");
            e
        })?
        .generate()
        .map_err(|e| {
            println!("Generate error");
            e
        })?;

    // print to stdout if no output arg is given
    if let Some(output_path) = args.next() {
        bindings.write_to_file(&output_path)?;
    } else {
        bindings.write(std::io::stdout())?;
    }

    Ok(())
}
