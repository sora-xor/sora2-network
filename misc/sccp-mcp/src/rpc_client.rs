use crate::error::{AppError, AppResult};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn rpc_call(url: &str, method: &str, params: Value) -> AppResult<Value> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let payload = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });

    let response = ureq::post(url)
        .set("content-type", "application/json")
        .send_json(payload)
        .map_err(|err| AppError::Rpc(format!("rpc request to {url} failed: {err}")))?;

    let body: Value = response.into_json().map_err(|err| {
        AppError::Rpc(format!("rpc response from {url} was not valid JSON: {err}"))
    })?;

    if let Some(err) = body.get("error") {
        return Err(AppError::Rpc(format!(
            "rpc error from {url} method {method}: {err}"
        )));
    }

    body.get("result")
        .cloned()
        .ok_or_else(|| AppError::Rpc(format!("rpc response from {url} missing result field")))
}
