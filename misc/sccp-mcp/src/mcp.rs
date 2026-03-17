use crate::error::AppError;
use crate::rpc_client::with_rpc_fairness_principal;
use crate::tools::{dispatch, tool_definitions_for_policy, ToolContext};
use blake2::{Blake2b512, Digest};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};
use std::sync::Mutex;
#[cfg(not(test))]
use std::thread;
use std::time::Duration;

#[cfg(not(test))]
const DEFAULT_AUTH_FAILURE_BASE_DELAY_MS: u64 = 50;
#[cfg(not(test))]
const DEFAULT_AUTH_FAILURE_MAX_DELAY_MS: u64 = 1_000;
const DEFAULT_AUTH_FAILURE_TRACKED_PRINCIPALS: usize = 2_048;
static AUTH_FAILURE_STREAK_BY_PRINCIPAL: Lazy<Mutex<BTreeMap<String, u32>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

fn method_requires_auth(method: &str) -> bool {
    matches!(method, "tools/list" | "tools/call")
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let mut diff = left_bytes.len() ^ right_bytes.len();
    let max_len = left_bytes.len().max(right_bytes.len());
    for idx in 0..max_len {
        let l = *left_bytes.get(idx).unwrap_or(&0);
        let r = *right_bytes.get(idx).unwrap_or(&0);
        diff |= usize::from(l ^ r);
    }
    diff == 0
}

fn parse_delay_ms_from_env(key: &str, default_ms: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default_ms)
}

fn parse_usize_from_env(key: &str, default_value: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default_value)
}

fn parse_bool_flag(raw: Option<String>, default: bool) -> bool {
    match raw
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
    {
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on") => true,
        Some(value) if matches!(value.as_str(), "0" | "false" | "no" | "off") => false,
        _ => default,
    }
}

fn requester_id_is_required() -> bool {
    parse_bool_flag(std::env::var("SCCP_MCP_REQUIRE_REQUESTER_ID").ok(), false)
}

fn effective_max_auth_token_bytes(ctx: &ToolContext) -> usize {
    ctx.config
        .auth
        .max_token_bytes
        .max(ctx.config.auth.min_required_token_bytes)
        .max(1)
}

fn auth_failure_delay_for_streak(streak: u32, base_ms: u64, max_ms: u64) -> Duration {
    if streak == 0 || base_ms == 0 || max_ms == 0 {
        return Duration::from_millis(0);
    }
    let step = u128::from(base_ms).saturating_mul(u128::from(streak));
    let bounded = step.min(u128::from(max_ms));
    Duration::from_millis(bounded as u64)
}

fn auth_failure_tracked_principal_limit() -> usize {
    parse_usize_from_env(
        "SCCP_MCP_AUTH_FAILURE_TRACKED_PRINCIPALS",
        DEFAULT_AUTH_FAILURE_TRACKED_PRINCIPALS,
    )
    .max(1)
}

fn auth_failure_principal_from_params(params: &Value, max_token_bytes: usize) -> String {
    if let Some(auth_token) = params.get("auth_token").and_then(Value::as_str) {
        if auth_token.as_bytes().len() > max_token_bytes {
            return "auth:oversized".to_owned();
        }
        return format!("auth:{}", principal_fingerprint(auth_token.as_bytes()));
    }
    "anonymous".to_owned()
}

fn record_auth_failure_streak(principal: &str, tracked_principal_limit: usize) -> u32 {
    let mut by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
        .lock()
        .expect("auth failure streak mutex should not be poisoned");
    if !by_principal.contains_key(principal) && by_principal.len() >= tracked_principal_limit {
        if let Some(oldest_key) = by_principal.keys().next().cloned() {
            by_principal.remove(&oldest_key);
            eprintln!(
                "SECURITY_AUTH_BACKOFF reason=auth_failure_streak_eviction principal={oldest_key} tracked_principal_limit={tracked_principal_limit}"
            );
        }
    }
    let current = *by_principal.get(principal).unwrap_or(&0);
    let next = current.saturating_add(1);
    by_principal.insert(principal.to_owned(), next);
    next
}

fn register_auth_failure_backoff(reason: &str, principal: &str) {
    let streak = record_auth_failure_streak(principal, auth_failure_tracked_principal_limit());
    #[cfg(test)]
    {
        let _ = reason;
        let _ = principal;
        let _ = streak;
        return;
    }

    #[cfg(not(test))]
    {
        let base_ms = parse_delay_ms_from_env(
            "SCCP_MCP_AUTH_FAILURE_BASE_DELAY_MS",
            DEFAULT_AUTH_FAILURE_BASE_DELAY_MS,
        );
        let max_ms = parse_delay_ms_from_env(
            "SCCP_MCP_AUTH_FAILURE_MAX_DELAY_MS",
            DEFAULT_AUTH_FAILURE_MAX_DELAY_MS,
        );
        let delay = auth_failure_delay_for_streak(streak, base_ms, max_ms);
        if !delay.is_zero() {
            eprintln!(
                "SECURITY_AUTH_BACKOFF reason={reason} principal={principal} streak={streak} delay_ms={}",
                delay.as_millis()
            );
            thread::sleep(delay);
        }
    }
}

fn reset_auth_failure_streak(principal: &str) {
    let mut by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
        .lock()
        .expect("auth failure streak mutex should not be poisoned");
    by_principal.remove(principal);
}

fn ensure_method_authorized(
    ctx: &ToolContext,
    method: &str,
    params: &Value,
) -> Result<(), AppError> {
    if !method_requires_auth(method) {
        return Ok(());
    }

    let Some(expected_token) = ctx.config.auth.required_token.as_deref() else {
        return Err(AppError::Config(
            "missing resolved auth token in runtime config".to_owned(),
        ));
    };
    let max_token_bytes = effective_max_auth_token_bytes(ctx);
    let auth_failure_principal = auth_failure_principal_from_params(params, max_token_bytes);
    let Some(provided_token) = params.get("auth_token").and_then(Value::as_str) else {
        register_auth_failure_backoff("missing_auth_token", &auth_failure_principal);
        return Err(AppError::InvalidArgument(
            "missing required params.auth_token".to_owned(),
        ));
    };
    if provided_token.as_bytes().len() > max_token_bytes {
        register_auth_failure_backoff("oversized_auth_token", &auth_failure_principal);
        return Err(AppError::InvalidArgument(format!(
            "params.auth_token exceeds max allowed bytes ({max_token_bytes})"
        )));
    }
    if !constant_time_eq(provided_token, expected_token) {
        register_auth_failure_backoff("invalid_auth_token", &auth_failure_principal);
        return Err(AppError::InvalidArgument(
            "invalid params.auth_token".to_owned(),
        ));
    }
    reset_auth_failure_streak(&auth_failure_principal);
    Ok(())
}

fn principal_fingerprint(input: &[u8]) -> String {
    let digest = Blake2b512::digest(input);
    hex::encode(&digest[..8])
}

fn requester_principal_from_params(params: &Value) -> String {
    if let Some(requester_id) = params.get("requester_id").and_then(Value::as_str) {
        return format!(
            "requester:{}",
            principal_fingerprint(requester_id.as_bytes())
        );
    }
    if let Some(auth_token) = params.get("auth_token").and_then(Value::as_str) {
        return format!("auth:{}", principal_fingerprint(auth_token.as_bytes()));
    }
    "anonymous".to_owned()
}

fn ensure_requester_id_if_required(params: &Value, required: bool) -> Result<(), AppError> {
    if !required {
        return Ok(());
    }
    match params.get("requester_id").and_then(Value::as_str) {
        Some(requester_id) if !requester_id.trim().is_empty() => Ok(()),
        _ => Err(AppError::InvalidArgument(
            "missing required params.requester_id".to_owned(),
        )),
    }
}

pub fn run_server(ctx: ToolContext) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    run_server_with_io(&ctx, &mut reader, &mut writer)
}

fn run_server_with_io<R: BufRead, W: Write>(
    ctx: &ToolContext,
    reader: &mut R,
    writer: &mut W,
) -> anyhow::Result<()> {
    loop {
        let message = read_frame(reader, ctx.config.limits.max_request_bytes)?;
        let Some(payload) = message else {
            break;
        };

        let request_value: Value = match serde_json::from_slice(&payload) {
            Ok(v) => v,
            Err(err) => {
                let response = error_response(Value::Null, -32700, &format!("parse error: {err}"));
                write_frame(writer, &response)?;
                continue;
            }
        };

        let Some(method) = request_value.get("method").and_then(Value::as_str) else {
            let id = request_value.get("id").cloned().unwrap_or(Value::Null);
            let response = error_response(id, -32600, "invalid request: missing method");
            write_frame(writer, &response)?;
            continue;
        };

        let id = request_value.get("id").cloned();
        let params = request_value
            .get("params")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let response = match method {
            "initialize" => id.map(|request_id| {
                success_response(
                    request_id,
                    json!({
                        "protocolVersion": "2024-11-05",
                        "serverInfo": {
                            "name": "sccp-mcp",
                            "version": env!("CARGO_PKG_VERSION"),
                        },
                        "capabilities": {
                            "tools": {
                                "listChanged": false
                            }
                        }
                    }),
                )
            }),
            "ping" => id.map(|request_id| success_response(request_id, json!({}))),
            "notifications/initialized" => None,
            "tools/list" => {
                id.map(
                    |request_id| match ensure_method_authorized(&ctx, "tools/list", &params) {
                        Ok(()) => success_response(
                            request_id,
                            json!({
                                "tools": tool_definitions_for_policy(&ctx.config.policy),
                            }),
                        ),
                        Err(err) => error_response(request_id, -32000, &err.to_string()),
                    },
                )
            }
            "tools/call" => id.map(|request_id| {
                let requester_principal = requester_principal_from_params(&params);
                let require_requester_id = requester_id_is_required();
                let outcome = ensure_method_authorized(&ctx, "tools/call", &params)
                    .and_then(|_| ensure_requester_id_if_required(&params, require_requester_id))
                    .and_then(|_| {
                        with_rpc_fairness_principal(&requester_principal, || {
                            handle_tool_call(&ctx, &params)
                        })
                    });
                match outcome {
                    Ok(value) => success_response(
                        request_id,
                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&value)
                                        .unwrap_or_else(|_| "{}".to_owned())
                                }
                            ],
                            "structuredContent": value,
                            "isError": false,
                        }),
                    ),
                    Err(err) => error_response(request_id, -32000, &err.to_string()),
                }
            }),
            _ => id.map(|request_id| error_response(request_id, -32601, "method not found")),
        };

        if let Some(resp) = response {
            write_frame(writer, &resp)?;
        }
    }

    Ok(())
}

fn handle_tool_call(ctx: &ToolContext, params: &Value) -> Result<Value, AppError> {
    let name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
        AppError::InvalidArgument("tools/call missing string params.name".to_owned())
    })?;
    let arguments = match params.get("arguments") {
        Some(Value::Object(_)) => params
            .get("arguments")
            .cloned()
            .expect("arguments object must exist"),
        Some(Value::Null) | None => json!({}),
        Some(_) => {
            return Err(AppError::InvalidArgument(
                "tools/call params.arguments must be object when provided".to_owned(),
            ))
        }
    };

    dispatch(ctx, name, &arguments)
}

fn read_frame<R: BufRead>(
    reader: &mut R,
    max_request_bytes: usize,
) -> anyhow::Result<Option<Vec<u8>>> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            if content_length.is_none() {
                return Ok(None);
            }
            anyhow::bail!("unexpected EOF while reading MCP headers");
        }

        if line == "\r\n" {
            break;
        }

        if let Some(value) = line.strip_prefix("Content-Length:") {
            if content_length.is_some() {
                anyhow::bail!("duplicate Content-Length header");
            }
            let trimmed = value.trim();
            if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
                anyhow::bail!("invalid Content-Length '{trimmed}': must be ASCII digits only");
            }
            let parsed = trimmed
                .parse::<usize>()
                .map_err(|err| anyhow::anyhow!("invalid Content-Length '{trimmed}': {err}"))?;
            content_length = Some(parsed);
        }
    }

    let len = content_length.ok_or_else(|| anyhow::anyhow!("missing Content-Length header"))?;
    if len > max_request_bytes {
        anyhow::bail!(
            "incoming MCP message exceeds max_request_bytes: {len} > {max_request_bytes}"
        );
    }

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

fn write_frame<W: Write>(writer: &mut W, payload: &Value) -> anyhow::Result<()> {
    let body = serde_json::to_vec(payload)?;
    writer.write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Auth, Config, DeploymentPolicy, Limits, Policy};
    use once_cell::sync::Lazy;
    use std::collections::BTreeMap;
    use std::io::{BufReader, Cursor};
    use std::sync::Mutex;

    static AUTH_FAILURE_TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn empty_ctx() -> ToolContext {
        ToolContext {
            config: Config {
                limits: Limits::default(),
                policy: Policy::default(),
                auth: Auth::default(),
                deployment: DeploymentPolicy::default(),
                networks: BTreeMap::new(),
            },
        }
    }

    fn authed_ctx(token: &str) -> ToolContext {
        ToolContext {
            config: Config {
                limits: Limits::default(),
                policy: Policy::default(),
                auth: Auth {
                    required_token: Some(token.to_owned()),
                    ..Auth::default()
                },
                deployment: DeploymentPolicy::default(),
                networks: BTreeMap::new(),
            },
        }
    }

    fn frame(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    fn run_server_with_input(ctx: &ToolContext, input: Vec<u8>) -> Vec<Value> {
        let mut reader = BufReader::new(Cursor::new(input));
        let mut output = Vec::<u8>::new();
        run_server_with_io(ctx, &mut reader, &mut output).expect("server loop should not fail");

        let mut output_reader = BufReader::new(Cursor::new(output));
        let mut responses = Vec::<Value>::new();
        loop {
            let frame = read_frame(&mut output_reader, 1_048_576)
                .expect("output frame parsing should succeed");
            let Some(frame) = frame else {
                break;
            };
            let response: Value =
                serde_json::from_slice(&frame).expect("response frame JSON should parse");
            responses.push(response);
        }
        responses
    }

    fn with_env_var(key: &str, value: Option<&str>, f: impl FnOnce()) {
        let previous = std::env::var(key).ok();
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        f();
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    fn clear_auth_failure_streak_state() {
        AUTH_FAILURE_STREAK_BY_PRINCIPAL
            .lock()
            .expect("auth failure streak mutex should not be poisoned")
            .clear();
    }

    fn with_auth_failure_state_lock(f: impl FnOnce()) {
        let _guard = AUTH_FAILURE_TEST_LOCK
            .lock()
            .expect("auth failure test lock should not be poisoned");
        clear_auth_failure_streak_state();
        f();
        clear_auth_failure_streak_state();
    }

    #[test]
    fn principal_fingerprint_is_stable_for_known_value() {
        assert_eq!(principal_fingerprint(b"auth-token"), "c726cecc919b33b8");
    }

    #[test]
    fn requester_principal_from_params_prefers_requester_id_then_auth_token() {
        let with_requester = json!({
            "requester_id": "gateway-user-42",
            "auth_token": "super-secret-token"
        });
        assert_eq!(
            requester_principal_from_params(&with_requester),
            format!("requester:{}", principal_fingerprint(b"gateway-user-42"))
        );

        let with_auth = json!({
            "auth_token": "super-secret-token"
        });
        assert_eq!(
            requester_principal_from_params(&with_auth),
            format!("auth:{}", principal_fingerprint(b"super-secret-token"))
        );

        let anonymous = json!({});
        assert_eq!(requester_principal_from_params(&anonymous), "anonymous");
    }

    #[test]
    fn requester_principal_from_params_falls_back_when_requester_id_not_string() {
        let with_non_string_requester = json!({
            "requester_id": 42,
            "auth_token": "super-secret-token"
        });
        assert_eq!(
            requester_principal_from_params(&with_non_string_requester),
            format!("auth:{}", principal_fingerprint(b"super-secret-token"))
        );
    }

    #[test]
    fn requester_principal_from_params_hashes_empty_requester_id() {
        let with_empty_requester = json!({
            "requester_id": ""
        });
        assert_eq!(
            requester_principal_from_params(&with_empty_requester),
            format!("requester:{}", principal_fingerprint(b""))
        );
    }

    #[test]
    fn requester_principal_from_params_hashes_multibyte_requester_id() {
        let requester_id = "用户-😀";
        let with_multibyte_requester = json!({
            "requester_id": requester_id
        });
        assert_eq!(
            requester_principal_from_params(&with_multibyte_requester),
            format!(
                "requester:{}",
                principal_fingerprint(requester_id.as_bytes())
            )
        );
    }

    #[test]
    fn requester_principal_from_params_falls_back_to_anonymous_when_auth_token_not_string() {
        let with_non_string_auth = json!({
            "auth_token": 42
        });
        assert_eq!(
            requester_principal_from_params(&with_non_string_auth),
            "anonymous"
        );
    }

    #[test]
    fn auth_failure_principal_from_params_prefers_auth_token_or_anonymous() {
        let with_auth = json!({
            "requester_id": "gateway-user-42",
            "auth_token": "super-secret-token"
        });
        assert_eq!(
            auth_failure_principal_from_params(&with_auth, 512),
            format!("auth:{}", principal_fingerprint(b"super-secret-token"))
        );

        let anonymous = json!({});
        assert_eq!(
            auth_failure_principal_from_params(&anonymous, 512),
            "anonymous"
        );
    }

    #[test]
    fn auth_failure_principal_from_params_ignores_non_string_token() {
        let non_string_auth = json!({
            "auth_token": 7
        });
        assert_eq!(
            auth_failure_principal_from_params(&non_string_auth, 512),
            "anonymous"
        );
    }

    #[test]
    fn auth_failure_principal_from_params_buckets_when_max_token_bytes_is_zero() {
        let with_auth = json!({
            "auth_token": "token"
        });
        assert_eq!(
            auth_failure_principal_from_params(&with_auth, 0),
            "auth:oversized"
        );
    }

    #[test]
    fn auth_failure_principal_from_params_accepts_token_at_exact_limit() {
        let exact_limit = json!({
            "auth_token": "a".repeat(512)
        });
        assert_eq!(
            auth_failure_principal_from_params(&exact_limit, 512),
            format!("auth:{}", principal_fingerprint("a".repeat(512).as_bytes()))
        );
    }

    #[test]
    fn auth_failure_principal_from_params_buckets_oversized_tokens() {
        let oversized = json!({
            "auth_token": "a".repeat(513)
        });
        assert_eq!(
            auth_failure_principal_from_params(&oversized, 512),
            "auth:oversized"
        );
    }

    #[test]
    fn parse_bool_flag_parses_expected_literals() {
        assert!(parse_bool_flag(Some("true".to_owned()), false));
        assert!(parse_bool_flag(Some("YES".to_owned()), false));
        assert!(parse_bool_flag(Some("1".to_owned()), false));
        assert!(parse_bool_flag(Some("on".to_owned()), false));
        assert!(!parse_bool_flag(Some("false".to_owned()), true));
        assert!(!parse_bool_flag(Some("NO".to_owned()), true));
        assert!(!parse_bool_flag(Some("0".to_owned()), true));
        assert!(!parse_bool_flag(Some("off".to_owned()), true));
        assert!(parse_bool_flag(Some("unknown".to_owned()), true));
        assert!(!parse_bool_flag(None, false));
    }

    #[test]
    fn parse_bool_flag_trims_whitespace() {
        assert!(parse_bool_flag(Some("  true  ".to_owned()), false));
        assert!(!parse_bool_flag(Some("\tOFF\t".to_owned()), true));
    }

    #[test]
    fn method_requires_auth_only_for_tool_methods() {
        assert!(method_requires_auth("tools/list"));
        assert!(method_requires_auth("tools/call"));
        assert!(!method_requires_auth("initialize"));
        assert!(!method_requires_auth("ping"));
        assert!(!method_requires_auth("notifications/initialized"));
    }

    #[test]
    fn ensure_requester_id_if_required_respects_toggle() {
        let missing = json!({});
        ensure_requester_id_if_required(&missing, false)
            .expect("requester_id should be optional when requirement disabled");
        let err = ensure_requester_id_if_required(&missing, true)
            .expect_err("requester_id should be required when requirement enabled");
        assert!(
            err.to_string()
                .contains("missing required params.requester_id"),
            "unexpected error: {err}"
        );

        let empty = json!({"requester_id": "   "});
        let err = ensure_requester_id_if_required(&empty, true)
            .expect_err("blank requester_id should be rejected when requirement enabled");
        assert!(
            err.to_string()
                .contains("missing required params.requester_id"),
            "unexpected error: {err}"
        );

        let valid = json!({"requester_id": "gateway-user-42"});
        ensure_requester_id_if_required(&valid, true)
            .expect("valid requester_id should satisfy requirement");
    }

    #[test]
    fn ensure_requester_id_if_required_rejects_non_string_value() {
        let non_string = json!({"requester_id": 99});
        let err = ensure_requester_id_if_required(&non_string, true)
            .expect_err("non-string requester_id must be rejected");
        assert!(
            err.to_string()
                .contains("missing required params.requester_id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn ensure_method_authorized_success_resets_only_matching_principal_streak() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let principal_for_token = format!("auth:{}", principal_fingerprint(b"token"));
            let other_principal = "auth:other";

            record_auth_failure_streak(&principal_for_token, 16);
            record_auth_failure_streak(other_principal, 16);

            ensure_method_authorized(&ctx, "tools/list", &json!({"auth_token":"token"}))
                .expect("matching auth token should pass and reset matching principal streak");

            let by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
                .lock()
                .expect("auth failure streak mutex should not be poisoned");
            assert!(
                !by_principal.contains_key(&principal_for_token),
                "successful auth should clear only the matching principal streak"
            );
            assert_eq!(
                by_principal.get(other_principal),
                Some(&1),
                "unrelated principal streak should remain untouched"
            );
        });
    }

    #[test]
    fn parse_usize_from_env_uses_default_for_invalid_values() {
        let key = format!("SCCP_MCP_TEST_USIZE_{}", std::process::id());
        std::env::remove_var(&key);
        assert_eq!(parse_usize_from_env(&key, 17), 17);

        std::env::set_var(&key, "99");
        assert_eq!(parse_usize_from_env(&key, 17), 99);

        std::env::set_var(&key, "invalid");
        assert_eq!(parse_usize_from_env(&key, 17), 17);

        std::env::set_var(&key, "-5");
        assert_eq!(parse_usize_from_env(&key, 17), 17);
        std::env::remove_var(&key);
    }

    #[test]
    fn record_auth_failure_streak_tracks_principals_independently() {
        with_auth_failure_state_lock(|| {
            let principal_a = "auth:principal_a";
            let principal_b = "auth:principal_b";

            assert_eq!(record_auth_failure_streak(principal_a, 16), 1);
            assert_eq!(record_auth_failure_streak(principal_a, 16), 2);
            assert_eq!(record_auth_failure_streak(principal_b, 16), 1);

            let by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
                .lock()
                .expect("auth failure streak mutex should not be poisoned");
            assert_eq!(by_principal.get(principal_a), Some(&2));
            assert_eq!(by_principal.get(principal_b), Some(&1));
        });
    }

    #[test]
    fn record_auth_failure_streak_evicts_oldest_key_at_capacity() {
        with_auth_failure_state_lock(|| {
            let principal_a = "auth:principal_a";
            let principal_b = "auth:principal_b";
            assert_eq!(record_auth_failure_streak(principal_a, 1), 1);
            assert_eq!(record_auth_failure_streak(principal_b, 1), 1);

            let by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
                .lock()
                .expect("auth failure streak mutex should not be poisoned");
            assert_eq!(by_principal.len(), 1);
            assert_eq!(by_principal.get(principal_b), Some(&1));
            assert!(!by_principal.contains_key(principal_a));
        });
    }

    #[test]
    fn reset_auth_failure_streak_removes_only_target_principal() {
        with_auth_failure_state_lock(|| {
            let principal_a = "auth:principal_a";
            let principal_b = "auth:principal_b";
            record_auth_failure_streak(principal_a, 16);
            record_auth_failure_streak(principal_b, 16);

            reset_auth_failure_streak(principal_a);

            let by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
                .lock()
                .expect("auth failure streak mutex should not be poisoned");
            assert!(!by_principal.contains_key(principal_a));
            assert_eq!(by_principal.get(principal_b), Some(&1));
        });
    }

    #[test]
    fn read_frame_parses_valid_message() {
        let body = "{\"jsonrpc\":\"2.0\"}";
        let bytes = frame(body);
        let mut reader = BufReader::new(Cursor::new(bytes));
        let parsed = read_frame(&mut reader, 1024).expect("frame should parse");
        assert_eq!(
            parsed.expect("frame should be present"),
            body.as_bytes(),
            "unexpected parsed payload"
        );
    }

    #[test]
    fn read_frame_rejects_oversized_message() {
        let body = "{\"jsonrpc\":\"2.0\"}";
        let bytes = frame(body);
        let mut reader = BufReader::new(Cursor::new(bytes));
        let err = read_frame(&mut reader, 4).expect_err("must reject oversized payload");
        assert!(
            err.to_string().contains("exceeds"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_requires_content_length_header() {
        let data = b"X-Test: 1\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024).expect_err("must reject missing content-length");
        assert!(
            err.to_string().contains("missing Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_lowercase_content_length_header() {
        let data = b"content-length: 2\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("header parsing is strict and must reject lowercase variant");
        assert!(
            err.to_string().contains("missing Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_non_numeric_content_length() {
        let data = b"Content-Length: nope\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024).expect_err("non-numeric content length must fail");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_non_ascii_digit_content_length() {
        // Arabic-Indic digit 2 should fail strict ASCII digit validation.
        let data = "Content-Length: ٢\r\n\r\n{}".as_bytes().to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err =
            read_frame(&mut reader, 1024).expect_err("non-ASCII content length must fail closed");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_content_length_with_internal_spaces() {
        let data = b"Content-Length: 1 0\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("content-length with internal spaces must fail");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_content_length_with_hex_notation() {
        let data = b"Content-Length: 0x2\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("hex-encoded content-length should fail closed");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_content_length_with_trailing_junk() {
        let data = b"Content-Length: 2abc\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("content-length containing trailing junk should fail closed");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_accepts_content_length_with_tabs() {
        let data = b"Content-Length:\t2\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let parsed = read_frame(&mut reader, 1024)
            .expect("content-length containing tab should parse after trimming");
        assert_eq!(parsed.expect("frame should be present"), b"{}");
    }

    #[test]
    fn read_frame_rejects_negative_content_length() {
        let data = b"Content-Length: -1\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024).expect_err("negative content length must fail");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_empty_content_length_value() {
        let data = b"Content-Length:\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024).expect_err("empty content length must fail");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_content_length_overflow() {
        let data = b"Content-Length: 184467440737095516160\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("overflowing Content-Length must fail parsing");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_plus_prefixed_content_length() {
        let data = b"Content-Length: +2\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("plus-prefixed Content-Length should fail closed");
        assert!(
            err.to_string().contains("invalid Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_duplicate_content_length_headers() {
        let data = b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024).expect_err("duplicate content length must fail");
        assert!(
            err.to_string().contains("duplicate Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_accepts_content_length_with_whitespace() {
        let body = "{}";
        let data = b"Content-Length:   2  \r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let parsed =
            read_frame(&mut reader, 1024).expect("content length with spaces should parse");
        assert_eq!(parsed.expect("frame should be present"), body.as_bytes());
    }

    #[test]
    fn read_frame_accepts_zero_length_body() {
        let data = b"Content-Length: 0\r\n\r\n".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let parsed = read_frame(&mut reader, 1024).expect("zero-length frame should parse");
        assert_eq!(parsed.expect("frame should be present"), Vec::<u8>::new());
    }

    #[test]
    fn read_frame_rejects_unexpected_eof_while_reading_headers() {
        let data = b"Content-Length: 5\r\n".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024).expect_err("truncated headers must fail");
        assert!(
            err.to_string()
                .contains("unexpected EOF while reading MCP headers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_returns_none_on_clean_eof() {
        let mut reader = BufReader::new(Cursor::new(Vec::<u8>::new()));
        let parsed = read_frame(&mut reader, 1024).expect("EOF should not error");
        assert!(parsed.is_none(), "expected None on clean EOF");
    }

    #[test]
    fn read_frame_accepts_payload_exactly_at_limit() {
        let body = "{\"ok\":true}";
        let bytes = frame(body);
        let mut reader = BufReader::new(Cursor::new(bytes));
        let parsed =
            read_frame(&mut reader, body.len()).expect("frame should parse at exact limit");
        assert_eq!(parsed.expect("frame should be present"), body.as_bytes());
    }

    #[test]
    fn read_frame_allows_extra_headers_when_content_length_present() {
        let body = "{}";
        let data = b"X-Test: 1\r\nContent-Length: 2\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let parsed =
            read_frame(&mut reader, body.len()).expect("frame should parse with extra headers");
        assert_eq!(parsed.expect("frame should be present"), body.as_bytes());
    }

    #[test]
    fn read_frame_rejects_truncated_body() {
        let data = b"Content-Length: 10\r\n\r\nabc".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024).expect_err("truncated body must fail");
        let text = err.to_string();
        assert!(
            text.contains("failed to fill whole buffer") || text.contains("unexpected end of file"),
            "unexpected error: {text}"
        );
    }

    #[test]
    fn read_frame_rejects_lf_only_header_termination() {
        let data = b"Content-Length: 2\n\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("LF-only header termination should fail closed");
        assert!(
            err.to_string()
                .contains("unexpected EOF while reading MCP headers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_content_length_header_with_name_whitespace() {
        let data = b"Content-Length : 2\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("header name whitespace variant should fail closed");
        assert!(
            err.to_string().contains("missing Content-Length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_frame_rejects_cr_only_header_termination() {
        let data = b"Content-Length: 2\r\r{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(data));
        let err = read_frame(&mut reader, 1024)
            .expect_err("CR-only header termination should fail closed");
        let text = err.to_string();
        assert!(
            text.contains("unexpected EOF while reading MCP headers")
                || text.contains("invalid Content-Length"),
            "unexpected error: {text}"
        );
    }

    #[test]
    fn write_frame_round_trip_with_read_frame() {
        let mut out = Vec::new();
        let payload = json!({"ok": true, "value": 7});
        write_frame(&mut out, &payload).expect("write_frame should succeed");

        let mut reader = BufReader::new(Cursor::new(out));
        let body = read_frame(&mut reader, 1024)
            .expect("read frame should succeed")
            .expect("frame should exist");
        let parsed: Value = serde_json::from_slice(&body).expect("json should parse");
        assert_eq!(parsed, payload, "payload should survive round-trip");
    }

    #[test]
    fn run_server_returns_parse_error_for_malformed_json() {
        let ctx = empty_ctx();
        let responses = run_server_with_input(&ctx, frame("{"));
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0]["jsonrpc"], json!("2.0"));
        assert_eq!(responses[0]["id"], Value::Null);
        assert_eq!(responses[0]["error"]["code"], json!(-32700));
        assert!(
            responses[0]["error"]["message"]
                .as_str()
                .expect("error message should be a string")
                .contains("parse error"),
            "expected parse error response"
        );
    }

    #[test]
    fn run_server_returns_invalid_request_when_method_missing() {
        let ctx = empty_ctx();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 7
        });
        let responses = run_server_with_input(&ctx, frame(&request.to_string()));
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0]["id"], json!(7));
        assert_eq!(responses[0]["error"]["code"], json!(-32600));
        assert_eq!(
            responses[0]["error"]["message"],
            json!("invalid request: missing method")
        );
    }

    #[test]
    fn run_server_returns_method_not_found_for_unknown_method() {
        let ctx = empty_ctx();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "unknown/method"
        });
        let responses = run_server_with_input(&ctx, frame(&request.to_string()));
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0]["id"], json!(3));
        assert_eq!(responses[0]["error"]["code"], json!(-32601));
        assert_eq!(responses[0]["error"]["message"], json!("method not found"));
    }

    #[test]
    fn run_server_emits_no_response_for_initialized_notification() {
        let ctx = empty_ctx();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let responses = run_server_with_input(&ctx, frame(&request.to_string()));
        assert!(
            responses.is_empty(),
            "notification should not produce JSON-RPC response frame"
        );
    }

    #[test]
    fn run_server_ping_without_id_emits_no_response() {
        let ctx = empty_ctx();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "ping",
            "params": {}
        });
        let responses = run_server_with_input(&ctx, frame(&request.to_string()));
        assert!(
            responses.is_empty(),
            "id-less request should be treated as notification and emit no response"
        );
    }

    #[test]
    fn run_server_processes_multiple_frames_in_sequence() {
        let ctx = empty_ctx();
        let req1 = json!({"jsonrpc":"2.0","id":1,"method":"ping","params":{}});
        let req2 = json!({"jsonrpc":"2.0","id":2,"method":"ping","params":{}});
        let mut input = Vec::new();
        input.extend_from_slice(&frame(&req1.to_string()));
        input.extend_from_slice(&frame(&req2.to_string()));

        let responses = run_server_with_input(&ctx, input);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["id"], json!(1));
        assert_eq!(responses[0]["result"], json!({}));
        assert_eq!(responses[1]["id"], json!(2));
        assert_eq!(responses[1]["result"], json!({}));
    }

    #[test]
    fn run_server_tools_list_auth_failure_propagates_error() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let request = json!({
                "jsonrpc": "2.0",
                "id": 11,
                "method": "tools/list",
                "params": {}
            });
            let responses = run_server_with_input(&ctx, frame(&request.to_string()));
            assert_eq!(responses.len(), 1);
            assert_eq!(responses[0]["id"], json!(11));
            assert_eq!(responses[0]["error"]["code"], json!(-32000));
            assert!(
                responses[0]["error"]["message"]
                    .as_str()
                    .expect("error message should be a string")
                    .contains("missing required params.auth_token"),
                "unexpected error payload: {}",
                responses[0]
            );
        });
    }

    #[test]
    fn run_server_tools_list_success_returns_tools() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let request = json!({
                "jsonrpc": "2.0",
                "id": 13,
                "method": "tools/list",
                "params": {
                    "auth_token": "token"
                }
            });
            let responses = run_server_with_input(&ctx, frame(&request.to_string()));
            assert_eq!(responses.len(), 1);
            assert_eq!(responses[0]["id"], json!(13));
            assert!(responses[0].get("result").is_some());
            assert!(
                responses[0]["result"]["tools"].is_array(),
                "tools/list success should include an array result.tools"
            );
        });
    }

    #[test]
    fn run_server_tools_call_argument_validation_propagates_error() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let request = json!({
                "jsonrpc": "2.0",
                "id": 12,
                "method": "tools/call",
                "params": {
                    "auth_token": "token",
                    "name": "sccp_list_networks",
                    "arguments": "not-an-object"
                }
            });
            let responses = run_server_with_input(&ctx, frame(&request.to_string()));
            assert_eq!(responses.len(), 1);
            assert_eq!(responses[0]["id"], json!(12));
            assert_eq!(responses[0]["error"]["code"], json!(-32000));
            assert!(
                responses[0]["error"]["message"]
                    .as_str()
                    .expect("error message should be a string")
                    .contains("params.arguments must be object"),
                "unexpected error payload: {}",
                responses[0]
            );
        });
    }

    #[test]
    fn run_server_tools_call_enforces_requester_id_when_required() {
        with_auth_failure_state_lock(|| {
            with_env_var("SCCP_MCP_REQUIRE_REQUESTER_ID", Some("true"), || {
                let ctx = authed_ctx("token");
                let request = json!({
                    "jsonrpc": "2.0",
                    "id": 14,
                    "method": "tools/call",
                    "params": {
                        "auth_token": "token",
                        "name": "sccp_list_networks",
                        "arguments": {}
                    }
                });
                let responses = run_server_with_input(&ctx, frame(&request.to_string()));
                assert_eq!(responses.len(), 1);
                assert_eq!(responses[0]["id"], json!(14));
                assert_eq!(responses[0]["error"]["code"], json!(-32000));
                assert!(
                    responses[0]["error"]["message"]
                        .as_str()
                        .expect("error message should be a string")
                        .contains("missing required params.requester_id"),
                    "unexpected error payload: {}",
                    responses[0]
                );
            });
        });
    }

    #[test]
    fn handle_tool_call_requires_name() {
        let ctx = empty_ctx();
        let err = handle_tool_call(&ctx, &json!({"arguments": {}}))
            .expect_err("missing tool name must fail");
        assert!(
            err.to_string().contains("missing string params.name"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn handle_tool_call_rejects_non_string_name() {
        let ctx = empty_ctx();
        let err = handle_tool_call(
            &ctx,
            &json!({
                "name": 7,
                "arguments": {}
            }),
        )
        .expect_err("non-string tool name must fail");
        assert!(
            err.to_string().contains("missing string params.name"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn handle_tool_call_defaults_arguments_when_absent() {
        let ctx = empty_ctx();
        let value = handle_tool_call(&ctx, &json!({"name": "sccp_list_networks"}))
            .expect("tools/call without arguments should default to empty object");
        assert_eq!(value, json!({"networks": []}));
    }

    #[test]
    fn handle_tool_call_rejects_non_object_arguments() {
        let ctx = empty_ctx();
        let err = handle_tool_call(
            &ctx,
            &json!({
                "name": "sccp_list_networks",
                "arguments": "not-an-object"
            }),
        )
        .expect_err("non-object arguments must fail");
        assert!(
            err.to_string().contains("params.arguments must be object"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn handle_tool_call_rejects_array_arguments() {
        let ctx = empty_ctx();
        let err = handle_tool_call(
            &ctx,
            &json!({
                "name": "sccp_list_networks",
                "arguments": []
            }),
        )
        .expect_err("array arguments must fail");
        assert!(
            err.to_string().contains("params.arguments must be object"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn handle_tool_call_rejects_boolean_arguments() {
        let ctx = empty_ctx();
        let err = handle_tool_call(
            &ctx,
            &json!({
                "name": "sccp_list_networks",
                "arguments": true
            }),
        )
        .expect_err("boolean arguments must fail");
        assert!(
            err.to_string().contains("params.arguments must be object"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn handle_tool_call_rejects_numeric_arguments() {
        let ctx = empty_ctx();
        let err = handle_tool_call(
            &ctx,
            &json!({
                "name": "sccp_list_networks",
                "arguments": 7
            }),
        )
        .expect_err("numeric arguments must fail");
        assert!(
            err.to_string().contains("params.arguments must be object"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn handle_tool_call_treats_null_arguments_as_empty_object() {
        let ctx = empty_ctx();
        let value = handle_tool_call(
            &ctx,
            &json!({
                "name": "sccp_list_networks",
                "arguments": null
            }),
        )
        .expect("null arguments should default to empty object");
        assert_eq!(value, json!({"networks": []}));
    }

    #[test]
    fn handle_tool_call_propagates_dispatch_errors() {
        let ctx = ToolContext {
            config: Config {
                limits: Limits::default(),
                policy: Policy {
                    allow_tools: vec!["does_not_exist".to_owned()],
                    deny_tools: vec![],
                },
                auth: Auth::default(),
                deployment: DeploymentPolicy::default(),
                networks: BTreeMap::new(),
            },
        };
        let err = handle_tool_call(
            &ctx,
            &json!({
                "name": "does_not_exist",
                "arguments": {}
            }),
        )
        .expect_err("unknown tools must fail");
        assert!(
            err.to_string().contains("unknown tool name"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn success_and_error_response_shapes_are_stable() {
        let success = success_response(json!(7), json!({"ok": true}));
        assert_eq!(success["jsonrpc"], json!("2.0"));
        assert_eq!(success["id"], json!(7));
        assert_eq!(success["result"]["ok"], json!(true));

        let error = error_response(json!(7), -32000, "boom");
        assert_eq!(error["jsonrpc"], json!("2.0"));
        assert_eq!(error["id"], json!(7));
        assert_eq!(error["error"]["code"], json!(-32000));
        assert_eq!(error["error"]["message"], json!("boom"));
    }

    #[test]
    fn ensure_method_authorized_skips_auth_for_non_tool_methods() {
        let ctx = authed_ctx("token");
        ensure_method_authorized(&ctx, "initialize", &json!({}))
            .expect("initialize should not require auth token");
    }

    #[test]
    fn ensure_method_authorized_rejects_missing_token_for_tool_methods() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let err = ensure_method_authorized(&ctx, "tools/list", &json!({}))
                .expect_err("tools/list should require auth token when configured");
            assert!(
                err.to_string()
                    .contains("missing required params.auth_token"),
                "unexpected error: {err}"
            );
        });
    }

    #[test]
    fn ensure_method_authorized_rejects_tool_methods_without_runtime_token() {
        let ctx = empty_ctx();
        let err = ensure_method_authorized(&ctx, "tools/list", &json!({"auth_token":"token"}))
            .expect_err("missing runtime token should fail closed");
        assert!(
            err.to_string().contains("missing resolved auth token"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn ensure_method_authorized_rejects_wrong_token_for_tool_methods() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let err = ensure_method_authorized(&ctx, "tools/call", &json!({"auth_token":"bad"}))
                .expect_err("wrong auth token should be rejected");
            assert!(
                err.to_string().contains("invalid params.auth_token"),
                "unexpected error: {err}"
            );
        });
    }

    #[test]
    fn ensure_method_authorized_records_hashed_principal_for_invalid_token() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let err = ensure_method_authorized(&ctx, "tools/call", &json!({"auth_token":"bad"}))
                .expect_err("wrong auth token should be rejected");
            assert!(
                err.to_string().contains("invalid params.auth_token"),
                "unexpected error: {err}"
            );

            let expected_principal = format!("auth:{}", principal_fingerprint(b"bad"));
            let by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
                .lock()
                .expect("auth failure streak mutex should not be poisoned");
            assert_eq!(
                by_principal.get(&expected_principal),
                Some(&1),
                "invalid token should increment hashed token principal streak"
            );
            assert!(
                !by_principal.contains_key("anonymous"),
                "invalid token should not be attributed to anonymous principal"
            );
        });
    }

    #[test]
    fn ensure_method_authorized_accepts_matching_token_for_tool_methods() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            ensure_method_authorized(&ctx, "tools/list", &json!({"auth_token":"token"}))
                .expect("matching auth token should pass");
        });
    }

    #[test]
    fn ensure_method_authorized_rejects_oversized_token_for_tool_methods() {
        with_auth_failure_state_lock(|| {
            let mut ctx = authed_ctx("token");
            ctx.config.auth.max_token_bytes = 8;
            ctx.config.auth.min_required_token_bytes = 1;
            let err =
                ensure_method_authorized(&ctx, "tools/call", &json!({"auth_token":"012345678"}))
                    .expect_err("oversized auth token should be rejected");
            assert!(
                err.to_string().contains("exceeds max allowed bytes"),
                "unexpected error: {err}"
            );
        });
    }

    #[test]
    fn constant_time_eq_matches_expected_results() {
        assert!(constant_time_eq("token", "token"));
        assert!(!constant_time_eq("token", "tokan"));
        assert!(!constant_time_eq("token", "token-extra"));
        assert!(!constant_time_eq("token-extra", "token"));
        assert!(!constant_time_eq("", "token"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn auth_failure_delay_for_streak_scales_and_caps() {
        assert_eq!(
            auth_failure_delay_for_streak(0, 50, 1000),
            Duration::from_millis(0)
        );
        assert_eq!(
            auth_failure_delay_for_streak(1, 50, 1000),
            Duration::from_millis(50)
        );
        assert_eq!(
            auth_failure_delay_for_streak(5, 50, 1000),
            Duration::from_millis(250)
        );
        assert_eq!(
            auth_failure_delay_for_streak(100, 50, 1000),
            Duration::from_millis(1000)
        );
        assert_eq!(
            auth_failure_delay_for_streak(3, 0, 1000),
            Duration::from_millis(0)
        );
        assert_eq!(
            auth_failure_delay_for_streak(3, 50, 0),
            Duration::from_millis(0)
        );
    }

    #[test]
    fn parse_delay_ms_from_env_uses_defaults_and_valid_values() {
        let key = format!("SCCP_MCP_TEST_DELAY_{}", std::process::id());
        std::env::remove_var(&key);
        assert_eq!(parse_delay_ms_from_env(&key, 123), 123);

        std::env::set_var(&key, "456");
        assert_eq!(parse_delay_ms_from_env(&key, 123), 456);

        std::env::set_var(&key, "invalid");
        assert_eq!(parse_delay_ms_from_env(&key, 123), 123);
        std::env::remove_var(&key);
    }

    #[test]
    fn requester_id_is_required_reads_env_flag() {
        let key = "SCCP_MCP_REQUIRE_REQUESTER_ID";
        let previous = std::env::var(key).ok();

        std::env::remove_var(key);
        assert!(
            !requester_id_is_required(),
            "missing env should default to false"
        );

        std::env::set_var(key, "true");
        assert!(requester_id_is_required());

        std::env::set_var(key, "0");
        assert!(!requester_id_is_required());

        std::env::set_var(key, "invalid");
        assert!(
            !requester_id_is_required(),
            "invalid env value should use default false"
        );

        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn auth_failure_tracked_principal_limit_has_minimum_of_one() {
        let key = "SCCP_MCP_AUTH_FAILURE_TRACKED_PRINCIPALS";
        let previous = std::env::var(key).ok();

        std::env::remove_var(key);
        assert_eq!(
            auth_failure_tracked_principal_limit(),
            DEFAULT_AUTH_FAILURE_TRACKED_PRINCIPALS
        );

        std::env::set_var(key, "0");
        assert_eq!(
            auth_failure_tracked_principal_limit(),
            1,
            "configured zero should clamp to minimum one principal"
        );

        std::env::set_var(key, "invalid");
        assert_eq!(
            auth_failure_tracked_principal_limit(),
            DEFAULT_AUTH_FAILURE_TRACKED_PRINCIPALS
        );

        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn effective_max_auth_token_bytes_respects_auth_bounds() {
        let mut ctx = empty_ctx();
        ctx.config.auth.min_required_token_bytes = 64;
        ctx.config.auth.max_token_bytes = 32;
        assert_eq!(
            effective_max_auth_token_bytes(&ctx),
            64,
            "effective max should be at least min_required_token_bytes"
        );

        ctx.config.auth.min_required_token_bytes = 0;
        ctx.config.auth.max_token_bytes = 0;
        assert_eq!(
            effective_max_auth_token_bytes(&ctx),
            1,
            "effective max should never drop below 1"
        );
    }

    #[test]
    fn ensure_method_authorized_records_anonymous_streak_on_missing_token() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let err = ensure_method_authorized(&ctx, "tools/list", &json!({}))
                .expect_err("missing token should fail authorization");
            assert!(
                err.to_string()
                    .contains("missing required params.auth_token"),
                "unexpected error: {err}"
            );

            let by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
                .lock()
                .expect("auth failure streak mutex should not be poisoned");
            assert_eq!(
                by_principal.get("anonymous"),
                Some(&1),
                "missing token should increment anonymous failure streak"
            );
        });
    }

    #[test]
    fn ensure_method_authorized_treats_non_string_token_as_missing() {
        with_auth_failure_state_lock(|| {
            let ctx = authed_ctx("token");
            let err = ensure_method_authorized(&ctx, "tools/list", &json!({"auth_token": 123}))
                .expect_err("non-string token should be treated as missing");
            assert!(
                err.to_string()
                    .contains("missing required params.auth_token"),
                "unexpected error: {err}"
            );

            let by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
                .lock()
                .expect("auth failure streak mutex should not be poisoned");
            assert_eq!(
                by_principal.get("anonymous"),
                Some(&1),
                "non-string token should increment anonymous failure streak"
            );
        });
    }

    #[test]
    fn ensure_method_authorized_records_oversized_bucket_streak() {
        with_auth_failure_state_lock(|| {
            let mut ctx = authed_ctx("token");
            ctx.config.auth.max_token_bytes = 8;
            ctx.config.auth.min_required_token_bytes = 1;

            let err =
                ensure_method_authorized(&ctx, "tools/call", &json!({"auth_token": "0123456789"}))
                    .expect_err("oversized token should fail authorization");
            assert!(
                err.to_string().contains("exceeds max allowed bytes"),
                "unexpected error: {err}"
            );

            let by_principal = AUTH_FAILURE_STREAK_BY_PRINCIPAL
                .lock()
                .expect("auth failure streak mutex should not be poisoned");
            assert_eq!(
                by_principal.get("auth:oversized"),
                Some(&1),
                "oversized token should increment dedicated oversized principal bucket"
            );
        });
    }
}
