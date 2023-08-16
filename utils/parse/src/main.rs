use codec::Decode;
use framenode_runtime::UncheckedExtrinsic;

fn main() {
    let mut args = std::env::args();
    let _ = args.next();
    let bytes = args.next().unwrap();
    let mut bytes = hex::decode(bytes).unwrap();
    bytes.insert(0, 0);
    let ext = UncheckedExtrinsic::decode(&mut &bytes[..]).unwrap();
    println!("{ext:?}");
}
