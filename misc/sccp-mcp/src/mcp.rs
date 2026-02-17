use crate::error::AppError;
use crate::tools::{dispatch, tool_definitions, ToolContext};
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};

pub fn run_server(ctx: ToolContext) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    loop {
        let message = read_frame(&mut reader, ctx.config.limits.max_request_bytes)?;
        let Some(payload) = message else {
            break;
        };

        let request_value: Value = match serde_json::from_slice(&payload) {
            Ok(v) => v,
            Err(err) => {
                let response = error_response(Value::Null, -32700, &format!("parse error: {err}"));
                write_frame(&mut writer, &response)?;
                continue;
            }
        };

        let Some(method) = request_value.get("method").and_then(Value::as_str) else {
            let id = request_value.get("id").cloned().unwrap_or(Value::Null);
            let response = error_response(id, -32600, "invalid request: missing method");
            write_frame(&mut writer, &response)?;
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
            "tools/list" => id.map(|request_id| {
                success_response(
                    request_id,
                    json!({
                        "tools": tool_definitions(),
                    }),
                )
            }),
            "tools/call" => id.map(|request_id| {
                let outcome = handle_tool_call(&ctx, &params);
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
            write_frame(&mut writer, &resp)?;
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
    use crate::config::{Config, Limits, Policy};
    use std::collections::BTreeMap;
    use std::io::{BufReader, Cursor};

    fn empty_ctx() -> ToolContext {
        ToolContext {
            config: Config {
                limits: Limits::default(),
                policy: Policy::default(),
                networks: BTreeMap::new(),
            },
        }
    }

    fn frame(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
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
        let ctx = empty_ctx();
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
}
