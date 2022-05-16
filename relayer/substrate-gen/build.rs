use std::path::PathBuf;
use std::str::FromStr;

fn main() {
    let metadata: Vec<u8> = framenode_runtime::Runtime::metadata().into();
    let out_dir = PathBuf::from_str(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).unwrap();
    std::fs::write(out_dir.join("src/bytes/metadata.scale"), metadata).unwrap();
    let workspace_root = PathBuf::from_str(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    println!("cargo:rerun-if-changed={}/runtime", workspace_root);
    println!("cargo:rerun-if-changed={}/pallets", workspace_root);
    println!("cargo:rerun-if-changed={}/common", workspace_root);
}
