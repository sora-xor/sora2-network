//! Minimal exact-Wasm adapter; every call explicitly rejects native fallback.
use crate::{hex_bytes, local_path, Result};
use codec::DecodeAll;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::{
    storage::StateVersion,
    traits::{CallContext, CodeExecutor, RuntimeCode, WrappedRuntimeCode},
    H256,
};
use std::{fs, io::Read, path::Path};
pub type Executor = sc_executor::WasmExecutor<sp_io::SubstrateHostFunctions>;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub file: String,
    pub sha256: String,
}
pub fn artifact(root: &Path, reference: &Artifact, maximum: usize) -> Result<Vec<u8>> {
    if hex_bytes(&reference.sha256)?.len() != 32 {
        return Err("invalid SHA-256 pin".into());
    }
    let path = local_path(root, &reference.file)?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > maximum || hex::encode(Sha256::digest(&bytes)) != reference.sha256 {
        return Err("artifact bound/checksum".into());
    }
    Ok(bytes)
}
pub fn root(ext: &mut sp_io::TestExternalities) -> H256 {
    H256::from_slice(&ext.execute_with(|| sp_io::storage::root(StateVersion::V0)))
}
pub fn call(
    executor: &Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    method: &str,
    data: &[u8],
) -> Result<Vec<u8>> {
    ext.register_extension(sp_core::traits::ReadRuntimeVersionExt::new(
        executor.clone(),
    ));
    let fetcher = WrappedRuntimeCode(std::borrow::Cow::Borrowed(code));
    let heap_pages = ext
        .execute_with(|| sp_io::storage::get(b":heappages"))
        .map(|bytes| u64::decode_all(&mut bytes.as_ref()).map_err(|e| e.to_string()))
        .transpose()?;
    let runtime = RuntimeCode {
        code_fetcher: &fetcher,
        heap_pages,
        hash: sp_io::hashing::blake2_256(code).to_vec(),
    };
    let (output, native) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        executor.call(
            &mut ext.ext(),
            &runtime,
            method,
            data,
            CallContext::Onchain,
        )
    }))
    .map_err(|_| format!("Wasm {method} panic"))?;
    if native {
        return Err("native fallback forbidden".into());
    }
    output.map_err(|e| format!("Wasm {method}: {e}"))
}
