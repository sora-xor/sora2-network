use crate::config::{Config, NetworkKind, NetworkProfile, Policy, MUTATING_TOOL_NAMES};
use crate::error::{AppError, AppResult};
use crate::payload::{
    message_id, parse_hex_fixed, parse_payload, validate_payload, SCCP_DOMAIN_BSC, SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA_KUSAMA, SCCP_DOMAIN_SORA_POLKADOT, SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
};
use crate::rpc_client::{rpc_call, with_rpc_fairness_scope};
use crate::sora_calls::{encode_attester_quorum_proof, encode_sora_call, supported_sora_calls};
use crate::substrate_storage::{
    decode_optional_bsc_header, decode_optional_bsc_params, decode_optional_bytes,
    decode_optional_tron_header, decode_optional_tron_params, decode_storage_bool,
    decode_token_state, double_map_key, map_key, storage_prefix,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use ethabi::{ethereum_types::U256, param_type::Reader, ParamType, Token};
use serde_json::{json, Value};

pub struct ToolContext {
    pub config: Config,
}

pub fn tool_definitions_for_policy(policy: &Policy) -> Vec<Value> {
    all_tool_definitions()
        .into_iter()
        .filter(|definition| {
            definition
                .get("name")
                .and_then(Value::as_str)
                .map(|name| policy.allows(name))
                .unwrap_or(false)
        })
        .collect()
}

fn all_tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "sccp_list_networks",
            "List configured network profiles available to this MCP server.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        tool(
            "sccp_health",
            "Check RPC connectivity and basic chain identity for one network or all networks.",
            json!({
                "type":"object",
                "properties": {
                    "network": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_get_message_id",
            "Compute canonical SCCP messageId = keccak256('sccp:burn:v1' || SCALE(BurnPayloadV1)).",
            json!({
                "type":"object",
                "required": ["payload"],
                "properties": {
                    "payload": {"type":"object"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_validate_payload",
            "Validate SCCP BurnPayloadV1 values and destination-specific constraints.",
            json!({
                "type":"object",
                "required": ["payload"],
                "properties": {
                    "payload": {"type":"object"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_encode_attester_quorum_proof",
            "Encode SCCP AttesterQuorum proof bytes (0x01 || SCALE(Vec<[u8;65]>)).",
            json!({
                "type":"object",
                "required": ["signatures"],
                "properties": {
                    "version": {"type":"integer"},
                    "signatures": {"type":"array", "items": {"type":"string"}}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_list_supported_calls",
            "List supported SCCP method surface for a network profile.",
            json!({
                "type":"object",
                "required": ["network"],
                "properties": {
                    "network": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_get_token_state",
            "Read SORA SCCP token state for an asset_id (32-byte hex).",
            json!({
                "type":"object",
                "required": ["network", "asset_id"],
                "properties": {
                    "network": {"type":"string"},
                    "asset_id": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_get_remote_token",
            "Read SORA SCCP remote token id by (asset_id, domain_id).",
            json!({
                "type":"object",
                "required": ["network", "asset_id", "domain_id"],
                "properties": {
                    "network": {"type":"string"},
                    "asset_id": {"type":"string"},
                    "domain_id": {"type":"integer"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_get_domain_endpoint",
            "Read SORA SCCP domain endpoint id by domain_id.",
            json!({
                "type":"object",
                "required": ["network", "domain_id"],
                "properties": {
                    "network": {"type":"string"},
                    "domain_id": {"type":"integer"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_preflight_activation",
            "Preflight-check SCCP activation readiness for an asset across required domains.",
            json!({
                "type":"object",
                "required": ["network", "asset_id"],
                "properties": {
                    "network": {"type":"string"},
                    "asset_id": {"type":"string"},
                    "domain_ids": {"type":"array", "items": {"type":"integer"}}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_get_light_client_state",
            "Read BSC/TRON light-client state from SORA SCCP storage.",
            json!({
                "type":"object",
                "required": ["network", "domain_id"],
                "properties": {
                    "network": {"type":"string"},
                    "domain_id": {"type":"integer"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sccp_get_message_status",
            "Check ProcessedInbound, AttestedOutbound and InvalidatedInbound flags for a message_id.",
            json!({
                "type":"object",
                "required": ["network", "source_domain", "message_id"],
                "properties": {
                    "network": {"type":"string"},
                    "source_domain": {"type":"integer"},
                    "message_id": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sora_sccp_build_call",
            "Build unsigned SORA SCCP call envelope for external signing.",
            json!({
                "type":"object",
                "required": ["network", "call_name", "args"],
                "properties": {
                    "network": {"type":"string"},
                    "call_name": {"type":"string"},
                    "args": {"type":"object"},
                    "pallet_index": {"type":"integer"},
                    "signer": {"type":"string"},
                    "nonce_mode": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sora_sccp_estimate_fee",
            "Estimate fee from a prebuilt unsigned/signed extrinsic hex using payment_queryInfo.",
            json!({
                "type":"object",
                "required": ["network", "extrinsic_hex"],
                "properties": {
                    "network": {"type":"string"},
                    "extrinsic_hex": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sora_sccp_submit_signed_extrinsic",
            "Submit pre-signed SCALE extrinsic hex to SORA.",
            json!({
                "type":"object",
                "required": ["network", "signed_extrinsic_hex"],
                "properties": {
                    "network": {"type":"string"},
                    "signed_extrinsic_hex": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "evm_sccp_read_contract",
            "Perform eth_call for SCCP contracts with optional ABI signature encoding/decoding.",
            json!({
                "type":"object",
                "required": ["network"],
                "properties": {
                    "network": {"type":"string"},
                    "to": {"type":"string"},
                    "data": {"type":"string"},
                    "signature": {"type":"string"},
                    "args": {"type":"array"},
                    "output_types": {"type":"array", "items": {"type":"string"}}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "evm_sccp_build_tx",
            "Build EVM transaction envelope for external signing; ABI encoding supported via signature+args.",
            json!({
                "type":"object",
                "required": ["network"],
                "properties": {
                    "network": {"type":"string"},
                    "from": {"type":"string"},
                    "to": {"type":"string"},
                    "data": {"type":"string"},
                    "signature": {"type":"string"},
                    "args": {"type":"array"},
                    "nonce": {"type":"string"},
                    "value": {"type":"string"},
                    "gas": {"type":"string"},
                    "max_fee_per_gas": {"type":"string"},
                    "max_priority_fee_per_gas": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "evm_sccp_submit_signed_tx",
            "Submit pre-signed raw EVM transaction hex.",
            json!({
                "type":"object",
                "required": ["network", "signed_tx_hex"],
                "properties": {
                    "network": {"type":"string"},
                    "signed_tx_hex": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sol_sccp_get_account",
            "Read Solana account info for SCCP program-related accounts.",
            json!({
                "type":"object",
                "required": ["network", "pubkey"],
                "properties": {
                    "network": {"type":"string"},
                    "pubkey": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sol_sccp_build_transaction",
            "Build Solana transaction template for external signer assembly.",
            json!({
                "type":"object",
                "required": ["network", "fee_payer", "recent_blockhash", "instructions"],
                "properties": {
                    "network": {"type":"string"},
                    "fee_payer": {"type":"string"},
                    "recent_blockhash": {"type":"string"},
                    "instructions": {"type":"array"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "sol_sccp_submit_signed_transaction",
            "Submit pre-signed Solana transaction (base64).",
            json!({
                "type":"object",
                "required": ["network", "signed_tx_base64"],
                "properties": {
                    "network": {"type":"string"},
                    "signed_tx_base64": {"type":"string"},
                    "encoding": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "ton_sccp_get_method",
            "Call TON JSON-RPC method for SCCP verifier/jetton introspection.",
            json!({
                "type":"object",
                "required": ["network", "method"],
                "properties": {
                    "network": {"type":"string"},
                    "method": {"type":"string"},
                    "params": {}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "ton_sccp_build_message",
            "Build TON message template for external signing and sending.",
            json!({
                "type":"object",
                "required": ["network", "opcode", "body"],
                "properties": {
                    "network": {"type":"string"},
                    "opcode": {},
                    "query_id": {},
                    "body": {}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "ton_sccp_submit_signed_message",
            "Submit pre-signed TON BOC payload.",
            json!({
                "type":"object",
                "required": ["network", "boc_base64"],
                "properties": {
                    "network": {"type":"string"},
                    "boc_base64": {"type":"string"},
                    "method": {"type":"string"}
                },
                "additionalProperties": false
            }),
        ),
    ]
}

pub fn dispatch(ctx: &ToolContext, name: &str, arguments: &Value) -> AppResult<Value> {
    if !ctx.config.policy.allows(name) {
        log_security_audit_event(name, arguments, false);
        return Err(AppError::ToolDenied(name.to_owned()));
    }
    log_security_audit_event(name, arguments, true);

    with_rpc_fairness_scope(name, || match name {
        "sccp_list_networks" => sccp_list_networks(ctx),
        "sccp_health" => sccp_health(ctx, arguments),
        "sccp_get_message_id" => sccp_get_message_id(arguments),
        "sccp_validate_payload" => sccp_validate_payload(arguments),
        "sccp_encode_attester_quorum_proof" => sccp_encode_attester_quorum_proof(arguments),
        "sccp_list_supported_calls" => sccp_list_supported_calls(ctx, arguments),
        "sccp_get_token_state" => sccp_get_token_state(ctx, arguments),
        "sccp_get_remote_token" => sccp_get_remote_token(ctx, arguments),
        "sccp_get_domain_endpoint" => sccp_get_domain_endpoint(ctx, arguments),
        "sccp_preflight_activation" => sccp_preflight_activation(ctx, arguments),
        "sccp_get_light_client_state" => sccp_get_light_client_state(ctx, arguments),
        "sccp_get_message_status" => sccp_get_message_status(ctx, arguments),
        "sora_sccp_build_call" => sora_sccp_build_call(ctx, arguments),
        "sora_sccp_estimate_fee" => sora_sccp_estimate_fee(ctx, arguments),
        "sora_sccp_submit_signed_extrinsic" => sora_sccp_submit_signed_extrinsic(ctx, arguments),
        "evm_sccp_read_contract" => evm_sccp_read_contract(ctx, arguments),
        "evm_sccp_build_tx" => evm_sccp_build_tx(ctx, arguments),
        "evm_sccp_submit_signed_tx" => evm_sccp_submit_signed_tx(ctx, arguments),
        "sol_sccp_get_account" => sol_sccp_get_account(ctx, arguments),
        "sol_sccp_build_transaction" => sol_sccp_build_transaction(ctx, arguments),
        "sol_sccp_submit_signed_transaction" => sol_sccp_submit_signed_transaction(ctx, arguments),
        "ton_sccp_get_method" => ton_sccp_get_method(ctx, arguments),
        "ton_sccp_build_message" => ton_sccp_build_message(ctx, arguments),
        "ton_sccp_submit_signed_message" => ton_sccp_submit_signed_message(ctx, arguments),
        other => Err(AppError::InvalidArgument(format!(
            "unknown tool name '{other}'"
        ))),
    })
}

fn is_high_risk_tool(name: &str) -> bool {
    MUTATING_TOOL_NAMES
        .iter()
        .any(|candidate| *candidate == name)
}

fn audit_network_hint(arguments: &Value) -> &str {
    arguments
        .get("network")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn log_security_audit_event(name: &str, arguments: &Value, allowed: bool) {
    if !is_high_risk_tool(name) {
        return;
    }
    let decision = if allowed { "allow" } else { "deny" };
    eprintln!(
        "SECURITY_AUDIT tool_decision={decision} tool={name} network={}",
        audit_network_hint(arguments)
    );
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn sccp_list_networks(ctx: &ToolContext) -> AppResult<Value> {
    let mut items = Vec::new();
    for (name, profile) in &ctx.config.networks {
        items.push(json!({
            "name": name,
            "kind": profile.kind.to_string(),
            "rpc_url": profile.rpc_url,
            "ws_url": profile.ws_url,
            "chain_id": profile.chain_id,
            "genesis_hash": profile.genesis_hash,
            "ss58_prefix": profile.ss58_prefix,
            "sccp_pallet_index": profile.sccp_pallet_index,
            "block_number_bytes": profile.block_number_bytes,
            "router_address": profile.router_address,
            "notes": profile.notes,
        }));
    }
    Ok(json!({ "networks": items }))
}

fn sccp_health(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let single_network = optional_string(args, "network")?;
    let targets: Vec<String> = if let Some(name) = single_network {
        vec![name]
    } else {
        ctx.config.list_network_names()
    };

    let mut results = Vec::new();
    for name in targets {
        let profile = ctx.config.network(&name)?;
        let status = match profile.kind {
            NetworkKind::Sora => sora_health(profile),
            NetworkKind::Evm => evm_health(profile),
            NetworkKind::Solana => solana_health(profile),
            NetworkKind::Ton => ton_health(profile),
        };

        match status {
            Ok(info) => results.push(json!({
                "network": name,
                "kind": profile.kind.to_string(),
                "ok": true,
                "info": info,
            })),
            Err(err) => results.push(json!({
                "network": name,
                "kind": profile.kind.to_string(),
                "ok": false,
                "error": err.to_string(),
            })),
        }
    }

    Ok(json!({ "health": results }))
}

fn sora_health(profile: &NetworkProfile) -> AppResult<Value> {
    let chain = rpc_call(&profile.rpc_url, "system_chain", json!([]))?;
    let name = rpc_call(&profile.rpc_url, "system_name", json!([]))?;
    let version = rpc_call(&profile.rpc_url, "system_version", json!([]))?;
    let head = rpc_call(&profile.rpc_url, "chain_getHeader", json!([]))?;
    let genesis_hash = rpc_call(&profile.rpc_url, "chain_getBlockHash", json!([0]))?;
    let expected_genesis = profile.genesis_hash.clone();
    let genesis_match = expected_genesis
        .as_deref()
        .and_then(|expected| genesis_hash.as_str().map(|actual| hex_eq(expected, actual)));
    Ok(json!({
        "chain": chain,
        "node_name": name,
        "node_version": version,
        "head": head,
        "genesis_hash": {
            "reported": genesis_hash,
            "expected": expected_genesis,
            "matches_expected": genesis_match,
        }
    }))
}

fn evm_health(profile: &NetworkProfile) -> AppResult<Value> {
    let chain_id = rpc_call(&profile.rpc_url, "eth_chainId", json!([]))?;
    let block_number = rpc_call(&profile.rpc_url, "eth_blockNumber", json!([]))?;
    let expected_chain_id = profile.chain_id;
    let matches_expected_chain_id = match (chain_id.as_str(), expected_chain_id) {
        (Some(reported), Some(expected)) => Some(parse_hex_u64(reported)? == expected),
        (_, None) => None,
        (None, Some(_)) => {
            return Err(AppError::Rpc(
                "eth_chainId did not return a hex string".to_owned(),
            ))
        }
    };
    Ok(json!({
        "chain_id": chain_id,
        "expected_chain_id": expected_chain_id,
        "matches_expected_chain_id": matches_expected_chain_id,
        "block_number": block_number,
    }))
}

fn solana_health(profile: &NetworkProfile) -> AppResult<Value> {
    let version = rpc_call(&profile.rpc_url, "getVersion", json!([]))?;
    let genesis_hash = rpc_call(&profile.rpc_url, "getGenesisHash", json!([]))?;
    let latest_blockhash = rpc_call(
        &profile.rpc_url,
        "getLatestBlockhash",
        json!([{"commitment":"finalized"}]),
    )?;
    let expected_genesis = profile.genesis_hash.clone();
    let genesis_match = expected_genesis
        .as_deref()
        .and_then(|expected| genesis_hash.as_str().map(|actual| expected == actual));
    Ok(json!({
        "version": version,
        "genesis_hash": {
            "reported": genesis_hash,
            "expected": expected_genesis,
            "matches_expected": genesis_match,
        },
        "latest_blockhash": latest_blockhash,
    }))
}

fn ton_health(profile: &NetworkProfile) -> AppResult<Value> {
    let masterchain = rpc_call(&profile.rpc_url, "getMasterchainInfo", json!([]))?;
    Ok(json!({
        "masterchain_info": masterchain,
    }))
}

fn sccp_get_message_id(args: &Value) -> AppResult<Value> {
    let payload = parse_payload(required_value(args, "payload")?)?;
    let digest = message_id(&payload)?;
    Ok(json!({
        "message_id": digest,
    }))
}

fn sccp_validate_payload(args: &Value) -> AppResult<Value> {
    let payload = parse_payload(required_value(args, "payload")?)?;
    let notes = validate_payload(&payload)?;
    Ok(json!({
        "valid": true,
        "notes": notes,
    }))
}

fn sccp_encode_attester_quorum_proof(args: &Value) -> AppResult<Value> {
    let version = match args.get("version") {
        Some(value) => {
            let raw = value.as_u64().ok_or_else(|| {
                AppError::InvalidArgument(
                    "field 'version' must be integer when provided".to_owned(),
                )
            })?;
            u8::try_from(raw).map_err(|_| {
                AppError::InvalidArgument("field 'version' does not fit u8".to_owned())
            })?
        }
        None => 1u8,
    };

    let signatures_value = args
        .get("signatures")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::InvalidArgument("missing array field 'signatures'".to_owned()))?;
    let mut signatures = Vec::with_capacity(signatures_value.len());
    for (idx, value) in signatures_value.iter().enumerate() {
        let text = value.as_str().ok_or_else(|| {
            AppError::InvalidArgument(format!("signatures[{idx}] must be hex string"))
        })?;
        signatures.push(parse_hex_fixed(text, 65, &format!("signatures[{idx}]"))?);
    }

    let proof = encode_attester_quorum_proof(&signatures, version)?;
    Ok(json!({
        "version": version,
        "signature_count": signatures.len(),
        "proof_hex": format!("0x{}", hex::encode(proof)),
    }))
}

fn sccp_list_supported_calls(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let network_name = required_string(args, "network")?;
    let profile = ctx.config.network(network_name)?;

    let calls = match profile.kind {
        NetworkKind::Sora => supported_sora_calls()
            .iter()
            .map(|spec| {
                json!({
                    "name": spec.name,
                    "call_index": spec.call_index,
                    "args": spec.args,
                })
            })
            .collect::<Vec<Value>>(),
        NetworkKind::Evm => vec![
            json!("burnToDomain(bytes32,uint256,uint32,bytes32)"),
            json!("mintFromProof(uint32,bytes,bytes)"),
            json!("deployToken(bytes32,string,string,uint8)"),
            json!("setDomainEndpoint(uint32,bytes)"),
            json!("setOutboundDomainPaused(uint32,bool)"),
            json!("setInboundDomainPaused(uint32,bool)"),
        ],
        NetworkKind::Solana => vec![
            json!("initialize"),
            json!("register_asset"),
            json!("burn"),
            json!("mint_from_proof"),
            json!("set_domain_pause"),
            json!("invalidate_inbound_message"),
        ],
        NetworkKind::Ton => vec![
            json!("SccpVerifierInitialize"),
            json!("SccpVerifierSubmitSignatureCommitment"),
            json!("SccpVerifierMintFromSoraProofV2"),
            json!("JettonBurnToDomain"),
            json!("JettonMintFromProof"),
        ],
    };

    Ok(json!({
        "network": network_name,
        "kind": profile.kind.to_string(),
        "supported_calls": calls,
    }))
}

fn sccp_get_token_state(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let asset_id = required_string(args, "asset_id")?;
    let asset_bytes = parse_hex_fixed(asset_id, 32, "asset_id")?;
    let key = map_key("Sccp", "Tokens", &asset_bytes);
    let raw = state_get_storage(&profile.rpc_url, &key)?;
    let decoded = decode_token_state(raw.as_deref())?;
    Ok(json!({
        "network": network_name,
        "asset_id": asset_id,
        "storage_key": key,
        "raw": raw,
        "token_state": decoded,
    }))
}

fn sccp_get_remote_token(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let asset_id = required_string(args, "asset_id")?;
    let asset_bytes = parse_hex_fixed(asset_id, 32, "asset_id")?;
    let domain_id = required_u32(args, "domain_id")?;
    let domain_bytes = domain_id.to_le_bytes();
    let key = double_map_key("Sccp", "RemoteToken", &asset_bytes, &domain_bytes);
    let raw = state_get_storage(&profile.rpc_url, &key)?;
    let decoded = decode_optional_bytes(raw.as_deref())?;

    Ok(json!({
        "network": network_name,
        "asset_id": asset_id,
        "domain_id": domain_id,
        "storage_key": key,
        "raw": raw,
        "remote_token_id": decoded,
    }))
}

fn sccp_get_domain_endpoint(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let domain_id = required_u32(args, "domain_id")?;
    let key = map_key("Sccp", "DomainEndpoint", &domain_id.to_le_bytes());
    let raw = state_get_storage(&profile.rpc_url, &key)?;
    let decoded = decode_optional_bytes(raw.as_deref())?;

    Ok(json!({
        "network": network_name,
        "domain_id": domain_id,
        "storage_key": key,
        "raw": raw,
        "domain_endpoint": decoded,
    }))
}

const SCCP_CORE_REMOTE_DOMAINS: [u32; 7] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
];

fn expected_remote_id_len(domain_id: u32) -> Option<usize> {
    match domain_id {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON => Some(20),
        SCCP_DOMAIN_SOL | SCCP_DOMAIN_TON | SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT => {
            Some(32)
        }
        _ => None,
    }
}

fn domain_ids_for_preflight(args: &Value) -> AppResult<Vec<u32>> {
    let Some(value) = args.get("domain_ids") else {
        return Ok(SCCP_CORE_REMOTE_DOMAINS.to_vec());
    };
    let Value::Array(items) = value else {
        return Err(AppError::InvalidArgument(
            "field 'domain_ids' must be an array when provided".to_owned(),
        ));
    };
    if items.is_empty() {
        return Err(AppError::InvalidArgument(
            "field 'domain_ids' must not be empty when provided".to_owned(),
        ));
    }

    let mut domains = Vec::with_capacity(items.len());
    for (idx, value) in items.iter().enumerate() {
        let raw = value.as_u64().ok_or_else(|| {
            AppError::InvalidArgument(format!("domain_ids[{idx}] must be an integer"))
        })?;
        let domain_id = u32::try_from(raw).map_err(|_| {
            AppError::InvalidArgument(format!("domain_ids[{idx}] does not fit u32"))
        })?;
        if expected_remote_id_len(domain_id).is_none() {
            return Err(AppError::InvalidArgument(format!(
                "domain_ids[{idx}] unsupported for SCCP activation preflight: {domain_id}"
            )));
        }
        if !domains.contains(&domain_id) {
            domains.push(domain_id);
        }
    }

    Ok(domains)
}

fn decoded_hex_len_bytes(value: Option<&str>) -> AppResult<Option<usize>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let normalized = raw.strip_prefix("0x").unwrap_or(raw);
    let bytes = hex::decode(normalized).map_err(|err| {
        AppError::Rpc(format!(
            "failed to decode hex value '{raw}' while preflighting: {err}"
        ))
    })?;
    Ok(Some(bytes.len()))
}

fn sccp_preflight_activation(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let asset_id = required_string(args, "asset_id")?;
    let asset_bytes = parse_hex_fixed(asset_id, 32, "asset_id")?;
    let token_state_key = map_key("Sccp", "Tokens", &asset_bytes);
    let token_state_raw = state_get_storage(&profile.rpc_url, &token_state_key)?;
    let token_state = decode_token_state(token_state_raw.as_deref())?;
    let token_state_status = token_state
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .map(str::to_owned);

    let domain_ids = domain_ids_for_preflight(args)?;
    let mut checks = Vec::with_capacity(domain_ids.len());
    let mut domains_ready = true;

    for domain_id in domain_ids.iter().copied() {
        let expected_len = expected_remote_id_len(domain_id).ok_or_else(|| {
            AppError::InvalidArgument(format!("unsupported domain for preflight: {domain_id}"))
        })?;

        let remote_key = double_map_key(
            "Sccp",
            "RemoteToken",
            &asset_bytes,
            &domain_id.to_le_bytes(),
        );
        let remote_raw = state_get_storage(&profile.rpc_url, &remote_key)?;
        let remote_token_id = decode_optional_bytes(remote_raw.as_deref())?;
        let remote_len = decoded_hex_len_bytes(remote_token_id.as_deref())?;
        let remote_present = remote_token_id.is_some();
        let remote_len_ok = remote_len.map(|len| len == expected_len).unwrap_or(false);

        let endpoint_key = map_key("Sccp", "DomainEndpoint", &domain_id.to_le_bytes());
        let endpoint_raw = state_get_storage(&profile.rpc_url, &endpoint_key)?;
        let domain_endpoint = decode_optional_bytes(endpoint_raw.as_deref())?;
        let endpoint_len = decoded_hex_len_bytes(domain_endpoint.as_deref())?;
        let endpoint_present = domain_endpoint.is_some();
        let endpoint_len_ok = endpoint_len.map(|len| len == expected_len).unwrap_or(false);

        let ready = remote_len_ok && endpoint_len_ok;
        domains_ready = domains_ready && ready;

        checks.push(json!({
            "domain_id": domain_id,
            "expected_len_bytes": expected_len,
            "ready": ready,
            "remote_token": {
                "present": remote_present,
                "len_bytes": remote_len,
                "len_ok": remote_len_ok,
                "value": remote_token_id,
                "storage_key": remote_key,
                "raw": remote_raw,
            },
            "domain_endpoint": {
                "present": endpoint_present,
                "len_bytes": endpoint_len,
                "len_ok": endpoint_len_ok,
                "value": domain_endpoint,
                "storage_key": endpoint_key,
                "raw": endpoint_raw,
            }
        }));
    }

    let token_state_ready = matches!(token_state_status.as_deref(), Some("pending"));

    Ok(json!({
        "network": network_name,
        "asset_id": asset_id,
        "domain_ids": domain_ids,
        "token_state": {
            "storage_key": token_state_key,
            "raw": token_state_raw,
            "decoded": token_state,
            "ready_for_activation": token_state_ready,
        },
        "domains_ready_for_activation": domains_ready,
        "ready_for_activation": token_state_ready && domains_ready,
        "checks": checks,
    }))
}

fn sccp_get_light_client_state(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let domain_id = required_u32(args, "domain_id")?;

    match domain_id {
        SCCP_DOMAIN_BSC => {
            let params = value_storage("BscParams", &profile.rpc_url)?;
            let head = value_storage("BscHead", &profile.rpc_url)?;
            let finalized = value_storage("BscFinalized", &profile.rpc_url)?;
            let validators = value_storage("BscValidators", &profile.rpc_url)?;

            Ok(json!({
                "network": network_name,
                "domain_id": domain_id,
                "params": decode_optional_bsc_params(params.as_deref())?,
                "head": decode_optional_bsc_header(head.as_deref())?,
                "finalized": decode_optional_bsc_header(finalized.as_deref())?,
                "validators": decode_optional_bytes(validators.as_deref())?,
            }))
        }
        SCCP_DOMAIN_TRON => {
            let params = value_storage("TronParams", &profile.rpc_url)?;
            let head = value_storage("TronHead", &profile.rpc_url)?;
            let finalized = value_storage("TronFinalized", &profile.rpc_url)?;
            let witnesses = value_storage("TronWitnesses", &profile.rpc_url)?;

            Ok(json!({
                "network": network_name,
                "domain_id": domain_id,
                "params": decode_optional_tron_params(params.as_deref())?,
                "head": decode_optional_tron_header(head.as_deref())?,
                "finalized": decode_optional_tron_header(finalized.as_deref())?,
                "witnesses": decode_optional_bytes(witnesses.as_deref())?,
            }))
        }
        _ => Err(AppError::InvalidArgument(
            "light-client state is currently available for domain_id 2 (BSC) and 5 (TRON)"
                .to_owned(),
        )),
    }
}

fn sccp_get_message_status(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let source_domain = required_u32(args, "source_domain")?;
    let message_id = required_string(args, "message_id")?;
    let message_bytes = parse_hex_fixed(message_id, 32, "message_id")?;

    let processed_key = map_key("Sccp", "ProcessedInbound", &message_bytes);
    let attested_key = map_key("Sccp", "AttestedOutbound", &message_bytes);
    let invalidated_key = double_map_key(
        "Sccp",
        "InvalidatedInbound",
        &source_domain.to_le_bytes(),
        &message_bytes,
    );

    let processed_raw = state_get_storage(&profile.rpc_url, &processed_key)?;
    let attested_raw = state_get_storage(&profile.rpc_url, &attested_key)?;
    let invalidated_raw = state_get_storage(&profile.rpc_url, &invalidated_key)?;

    let processed = decode_storage_bool(processed_raw.as_deref())?;
    let attested = decode_storage_bool(attested_raw.as_deref())?;
    let invalidated = decode_storage_bool(invalidated_raw.as_deref())?;

    Ok(json!({
        "network": network_name,
        "source_domain": source_domain,
        "message_id": message_id,
        "processed_inbound": processed,
        "attested_outbound": attested,
        "invalidated_inbound": invalidated,
        "keys": {
            "processed_inbound": processed_key,
            "attested_outbound": attested_key,
            "invalidated_inbound": invalidated_key,
        }
    }))
}

fn sora_sccp_build_call(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let call_name = required_string(args, "call_name")?;
    let call_args = required_value(args, "args")?.clone();
    let block_number_bytes = profile.block_number_bytes;
    let pallet_index = if let Some(explicit) = args.get("pallet_index").and_then(Value::as_u64) {
        u8::try_from(explicit).map_err(|_| {
            AppError::InvalidArgument("field 'pallet_index' does not fit u8".to_owned())
        })?
    } else if let Some(configured) = profile.sccp_pallet_index {
        configured
    } else {
        return Err(AppError::InvalidArgument(
            "SORA profile is missing sccp_pallet_index and no 'pallet_index' argument was provided"
                .to_owned(),
        ));
    };

    let encoded = encode_sora_call(
        call_name,
        &call_args,
        pallet_index,
        block_number_bytes,
        ctx.config.limits.max_call_bytes,
        ctx.config.limits.max_proof_bytes,
    )?;

    let signer = optional_string(args, "signer")?;
    let nonce_mode = optional_string(args, "nonce_mode")?.unwrap_or_else(|| "pending".to_owned());

    let nonce_info = if let Some(signer_account) = signer.as_deref() {
        let nonce = rpc_call(
            &profile.rpc_url,
            "system_accountNextIndex",
            json!([signer_account]),
        )?;
        Some(json!({ "signer": signer_account, "nonce": nonce, "mode": nonce_mode }))
    } else {
        None
    };

    Ok(json!({
        "network": network_name,
        "pallet": "Sccp",
        "call": encoded.name,
        "pallet_index": encoded.pallet_index,
        "call_index": encoded.call_index,
        "block_number_bytes": block_number_bytes,
        "args": call_args,
        "args_hex": format!("0x{}", hex::encode(&encoded.arg_bytes)),
        "call_data_hex": format!("0x{}", hex::encode(&encoded.call_data)),
        "call_data_len": encoded.call_data.len(),
        "nonce_hint": nonce_info,
        "external_signing_required": true,
        "notes": [
            "This MCP server does not hold keys.",
            "Use call_data_hex with your external signer stack to build/sign extrinsic, then submit via sora_sccp_submit_signed_extrinsic."
        ]
    }))
}

fn sora_sccp_estimate_fee(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let extrinsic_hex = required_string(args, "extrinsic_hex")?;
    ensure_hex_string(extrinsic_hex, "extrinsic_hex")?;

    let info = rpc_call(
        &profile.rpc_url,
        "payment_queryInfo",
        json!([extrinsic_hex, "latest"]),
    )?;
    Ok(json!({
        "network": network_name,
        "fee_info": info,
    }))
}

fn sora_sccp_submit_signed_extrinsic(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_sora_network(ctx, args)?;
    let signed_extrinsic_hex = required_string(args, "signed_extrinsic_hex")?;
    ensure_hex_string(signed_extrinsic_hex, "signed_extrinsic_hex")?;

    let tx_hash = rpc_call(
        &profile.rpc_url,
        "author_submitExtrinsic",
        json!([signed_extrinsic_hex]),
    )?;
    Ok(json!({
        "network": network_name,
        "tx_hash": tx_hash,
    }))
}

fn evm_sccp_read_contract(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_evm_network(ctx, args)?;
    let to = optional_string(args, "to")?
        .or_else(|| profile.router_address.clone())
        .ok_or_else(|| {
            AppError::InvalidArgument("missing 'to' and no router_address in config".to_owned())
        })?;

    let data = if let Some(data_hex) = optional_string(args, "data")? {
        ensure_hex_string(&data_hex, "data")?;
        data_hex
    } else {
        let signature = required_string(args, "signature")?;
        let call_args = args.get("args").cloned().unwrap_or_else(|| json!([]));
        encode_abi_call(signature, &call_args)?
    };

    let result = rpc_call(
        &profile.rpc_url,
        "eth_call",
        json!([{"to": to, "data": data}, "latest"]),
    )?;

    let decoded = if let Some(output_types_value) = args.get("output_types") {
        let output_types = parse_output_types(output_types_value)?;
        decode_abi_output(&result, &output_types)?
    } else {
        Value::Null
    };

    Ok(json!({
        "network": network_name,
        "raw_result": result,
        "decoded": decoded,
    }))
}

fn evm_sccp_build_tx(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_evm_network(ctx, args)?;

    let to = optional_string(args, "to")?
        .or_else(|| profile.router_address.clone())
        .ok_or_else(|| {
            AppError::InvalidArgument("missing 'to' and no router_address in config".to_owned())
        })?;

    let data = if let Some(data_hex) = optional_string(args, "data")? {
        ensure_hex_string(&data_hex, "data")?;
        data_hex
    } else if let Some(signature) = optional_string(args, "signature")? {
        let call_args = args.get("args").cloned().unwrap_or_else(|| json!([]));
        encode_abi_call(&signature, &call_args)?
    } else {
        "0x".to_owned()
    };

    let from = optional_string(args, "from")?;
    let nonce = if let Some(nonce) = optional_string(args, "nonce")? {
        ensure_hex_or_decimal(&nonce, "nonce")?;
        Some(nonce)
    } else if let Some(from_addr) = from.as_deref() {
        Some(stringify_json(
            &rpc_call(
                &profile.rpc_url,
                "eth_getTransactionCount",
                json!([from_addr, "pending"]),
            )?,
            "eth_getTransactionCount result",
        )?)
    } else {
        None
    };

    let chain_id = if let Some(id) = profile.chain_id {
        Some(format!("0x{:x}", id))
    } else {
        Some(stringify_json(
            &rpc_call(&profile.rpc_url, "eth_chainId", json!([]))?,
            "eth_chainId",
        )?)
    };

    let value = optional_string(args, "value")?.unwrap_or_else(|| "0x0".to_owned());
    ensure_hex_or_decimal(&value, "value")?;

    let tx = json!({
        "from": from,
        "to": to,
        "data": data,
        "nonce": nonce,
        "value": value,
        "gas": optional_string(args, "gas")?,
        "maxFeePerGas": optional_string(args, "max_fee_per_gas")?,
        "maxPriorityFeePerGas": optional_string(args, "max_priority_fee_per_gas")?,
        "chainId": chain_id,
    });

    Ok(json!({
        "network": network_name,
        "unsigned_tx": tx,
        "external_signing_required": true,
    }))
}

fn evm_sccp_submit_signed_tx(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_evm_network(ctx, args)?;
    let signed_tx_hex = required_string(args, "signed_tx_hex")?;
    ensure_hex_string(signed_tx_hex, "signed_tx_hex")?;
    let tx_hash = rpc_call(
        &profile.rpc_url,
        "eth_sendRawTransaction",
        json!([signed_tx_hex]),
    )?;

    Ok(json!({
        "network": network_name,
        "tx_hash": tx_hash,
    }))
}

fn sol_sccp_get_account(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_solana_network(ctx, args)?;
    let pubkey = required_string(args, "pubkey")?;
    let account_info = rpc_call(
        &profile.rpc_url,
        "getAccountInfo",
        json!([pubkey, {"encoding": "base64"}]),
    )?;
    Ok(json!({
        "network": network_name,
        "account": account_info,
    }))
}

fn sol_sccp_build_transaction(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, _) = require_solana_network(ctx, args)?;
    let fee_payer = required_string(args, "fee_payer")?;
    let recent_blockhash = required_string(args, "recent_blockhash")?;
    let instructions = required_value(args, "instructions")?.clone();

    Ok(json!({
        "network": network_name,
        "transaction_template": {
            "fee_payer": fee_payer,
            "recent_blockhash": recent_blockhash,
            "instructions": instructions,
        },
        "external_signing_required": true,
        "notes": [
            "This server returns an instruction template; compile/sign with your Solana signer stack.",
            "Submit resulting base64 transaction via sol_sccp_submit_signed_transaction."
        ]
    }))
}

fn sol_sccp_submit_signed_transaction(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_solana_network(ctx, args)?;
    let signed_tx_base64 = required_string(args, "signed_tx_base64")?;
    let encoding = optional_string(args, "encoding")?.unwrap_or_else(|| "base64".to_owned());

    let signature = rpc_call(
        &profile.rpc_url,
        "sendTransaction",
        json!([
            signed_tx_base64,
            {
                "encoding": encoding,
                "skipPreflight": false,
                "preflightCommitment": "confirmed"
            }
        ]),
    )?;

    Ok(json!({
        "network": network_name,
        "signature": signature,
    }))
}

fn ensure_ton_read_method(method: &str) -> AppResult<()> {
    let trimmed = method.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidArgument(
            "TON method must be a non-empty string".to_owned(),
        ));
    }
    if trimmed != method {
        return Err(AppError::InvalidArgument(
            "TON method must not include leading/trailing whitespace".to_owned(),
        ));
    }

    let lower = trimmed.to_ascii_lowercase();
    let is_allowed = lower.starts_with("get") || lower == "rungetmethod";
    if !is_allowed {
        return Err(AppError::InvalidArgument(format!(
            "TON read tool only allows read-only methods (get* or runGetMethod), got '{method}'"
        )));
    }

    Ok(())
}

fn ton_sccp_get_method(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_ton_network(ctx, args)?;
    let method = required_string(args, "method")?;
    ensure_ton_read_method(method)?;
    let params = args.get("params").cloned().unwrap_or_else(|| json!([]));
    let result = rpc_call(&profile.rpc_url, method, params)?;
    Ok(json!({
        "network": network_name,
        "method": method,
        "result": result,
    }))
}

fn ton_sccp_build_message(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, _) = require_ton_network(ctx, args)?;
    let opcode = required_value(args, "opcode")?.clone();
    let body = required_value(args, "body")?.clone();
    let query_id = args.get("query_id").cloned().unwrap_or(Value::Null);

    let message_object = json!({
        "opcode": opcode,
        "query_id": query_id,
        "body": body,
    });

    let serialized = serde_json::to_vec(&message_object).map_err(|err| {
        AppError::InvalidArgument(format!("failed to serialize TON message template: {err}"))
    })?;

    Ok(json!({
        "network": network_name,
        "message_template": message_object,
        "message_template_base64": BASE64_STANDARD.encode(serialized),
        "external_signing_required": true,
        "notes": [
            "This is a template for your TON signer pipeline.",
            "Submit produced BOC via ton_sccp_submit_signed_message."
        ]
    }))
}

fn ton_sccp_submit_signed_message(ctx: &ToolContext, args: &Value) -> AppResult<Value> {
    let (network_name, profile) = require_ton_network(ctx, args)?;
    let boc_base64 = required_string(args, "boc_base64")?;
    let method = optional_string(args, "method")?.unwrap_or_else(|| "sendBoc".to_owned());

    let result = rpc_call(&profile.rpc_url, &method, json!([boc_base64]))?;
    Ok(json!({
        "network": network_name,
        "method": method,
        "result": result,
    }))
}

fn require_sora_network<'a>(
    ctx: &'a ToolContext,
    args: &Value,
) -> AppResult<(String, &'a NetworkProfile)> {
    let network_name = required_string(args, "network")?.to_owned();
    let profile = ctx.config.network(&network_name)?;
    if profile.kind != NetworkKind::Sora {
        return Err(AppError::UnsupportedNetworkKind(profile.kind.to_string()));
    }
    Ok((network_name, profile))
}

fn require_evm_network<'a>(
    ctx: &'a ToolContext,
    args: &Value,
) -> AppResult<(String, &'a NetworkProfile)> {
    let network_name = required_string(args, "network")?.to_owned();
    let profile = ctx.config.network(&network_name)?;
    if profile.kind != NetworkKind::Evm {
        return Err(AppError::UnsupportedNetworkKind(profile.kind.to_string()));
    }
    Ok((network_name, profile))
}

fn require_solana_network<'a>(
    ctx: &'a ToolContext,
    args: &Value,
) -> AppResult<(String, &'a NetworkProfile)> {
    let network_name = required_string(args, "network")?.to_owned();
    let profile = ctx.config.network(&network_name)?;
    if profile.kind != NetworkKind::Solana {
        return Err(AppError::UnsupportedNetworkKind(profile.kind.to_string()));
    }
    Ok((network_name, profile))
}

fn require_ton_network<'a>(
    ctx: &'a ToolContext,
    args: &Value,
) -> AppResult<(String, &'a NetworkProfile)> {
    let network_name = required_string(args, "network")?.to_owned();
    let profile = ctx.config.network(&network_name)?;
    if profile.kind != NetworkKind::Ton {
        return Err(AppError::UnsupportedNetworkKind(profile.kind.to_string()));
    }
    Ok((network_name, profile))
}

fn required_string<'a>(value: &'a Value, field: &str) -> AppResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing string field '{field}'")))
}

fn optional_string<'a>(value: &'a Value, field: &str) -> AppResult<Option<String>> {
    match value.get(field) {
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(AppError::InvalidArgument(format!(
            "field '{field}' must be a string when provided"
        ))),
    }
}

fn required_u32(value: &Value, field: &str) -> AppResult<u32> {
    let raw = value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing integer field '{field}'")))?;
    u32::try_from(raw)
        .map_err(|_| AppError::InvalidArgument(format!("field '{field}' does not fit u32")))
}

fn required_value<'a>(value: &'a Value, field: &str) -> AppResult<&'a Value> {
    value
        .get(field)
        .ok_or_else(|| AppError::InvalidArgument(format!("missing field '{field}'")))
}

fn ensure_hex_string(value: &str, field: &str) -> AppResult<()> {
    if !value.starts_with("0x") {
        return Err(AppError::InvalidArgument(format!(
            "field '{field}' must be 0x-prefixed hex"
        )));
    }
    let body = value.strip_prefix("0x").unwrap_or(value);
    if body.is_empty() {
        return Ok(());
    }
    hex::decode(body).map_err(|err| {
        AppError::InvalidArgument(format!("field '{field}' must be valid hex: {err}"))
    })?;
    Ok(())
}

fn ensure_hex_or_decimal(value: &str, field: &str) -> AppResult<()> {
    if value.starts_with("0x") {
        ensure_hex_string(value, field)
    } else {
        value.parse::<u128>().map_err(|err| {
            AppError::InvalidArgument(format!(
                "field '{field}' must be decimal or 0x hex integer: {err}"
            ))
        })?;
        Ok(())
    }
}

fn state_get_storage(rpc_url: &str, key: &str) -> AppResult<Option<String>> {
    ensure_hex_string(key, "storage key")?;
    let raw = rpc_call(rpc_url, "state_getStorage", json!([key]))?;
    match raw {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text)),
        other => Err(AppError::Rpc(format!(
            "state_getStorage returned non-string value: {other}"
        ))),
    }
}

fn value_storage(storage_item: &str, rpc_url: &str) -> AppResult<Option<String>> {
    let key = format!("0x{}", hex::encode(storage_prefix("Sccp", storage_item)));
    state_get_storage(rpc_url, &key)
}

fn encode_abi_call(signature: &str, args: &Value) -> AppResult<String> {
    let (name, param_types) = parse_signature(signature)?;
    let arg_values = args.as_array().ok_or_else(|| {
        AppError::InvalidArgument("'args' must be an array for ABI encoding".to_owned())
    })?;

    if arg_values.len() != param_types.len() {
        return Err(AppError::InvalidArgument(format!(
            "ABI argument count mismatch for {signature}: expected {}, got {}",
            param_types.len(),
            arg_values.len()
        )));
    }

    let mut tokens = Vec::with_capacity(param_types.len());
    for (param_type, value) in param_types.iter().zip(arg_values) {
        tokens.push(parse_abi_token(param_type, value)?);
    }

    let selector = ethabi::short_signature(&name, &param_types);
    let encoded = ethabi::encode(&tokens);
    let mut data = Vec::with_capacity(4 + encoded.len());
    data.extend_from_slice(&selector);
    data.extend_from_slice(&encoded);
    Ok(format!("0x{}", hex::encode(data)))
}

fn parse_signature(signature: &str) -> AppResult<(String, Vec<ParamType>)> {
    let open = signature.find('(').ok_or_else(|| {
        AppError::InvalidArgument(format!("invalid signature '{signature}' (missing '(')"))
    })?;
    let close = signature.rfind(')').ok_or_else(|| {
        AppError::InvalidArgument(format!("invalid signature '{signature}' (missing ')')"))
    })?;
    if close <= open {
        return Err(AppError::InvalidArgument(format!(
            "invalid signature '{signature}'"
        )));
    }
    if !signature[(close + 1)..].trim().is_empty() {
        return Err(AppError::InvalidArgument(format!(
            "invalid signature '{signature}' (trailing characters after ')')"
        )));
    }

    let name = signature[0..open].trim();
    if name.is_empty() {
        return Err(AppError::InvalidArgument(
            "function name in signature cannot be empty".to_owned(),
        ));
    }
    if !is_valid_abi_function_name(name) {
        return Err(AppError::InvalidArgument(format!(
            "invalid function name '{name}' in signature"
        )));
    }

    let params_str = signature[(open + 1)..close].trim();
    if params_str.is_empty() {
        return Ok((name.to_owned(), Vec::new()));
    }

    let mut params = Vec::new();
    for part in split_params(params_str) {
        if part.is_empty() {
            return Err(AppError::InvalidArgument(format!(
                "invalid signature '{signature}' (empty parameter type)"
            )));
        }
        let compact: String = part
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect();
        if compact.contains(",,") || compact.contains("(,") || compact.contains(",)") {
            return Err(AppError::InvalidArgument(format!(
                "invalid signature '{signature}' (empty parameter type)"
            )));
        }
        let param_type = Reader::read(part).map_err(|err| {
            AppError::InvalidArgument(format!(
                "invalid ABI param type '{part}' in signature '{signature}': {err}"
            ))
        })?;
        params.push(param_type);
    }

    Ok((name.to_owned(), params))
}

fn split_params(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(input[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(input[start..].trim());
    parts
}

fn is_valid_abi_function_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn parse_abi_token(param_type: &ParamType, value: &Value) -> AppResult<Token> {
    match param_type {
        ParamType::Address => {
            let text = value.as_str().ok_or_else(|| {
                AppError::InvalidArgument("address argument must be string".to_owned())
            })?;
            let bytes = parse_hex_fixed(text, 20, "address")?;
            Ok(Token::Address(ethabi::ethereum_types::Address::from_slice(
                &bytes,
            )))
        }
        ParamType::Uint(_) => {
            let parsed = parse_value_u256(value)?;
            Ok(Token::Uint(parsed))
        }
        ParamType::Int(_) => {
            let parsed = parse_value_u256(value)?;
            Ok(Token::Int(parsed))
        }
        ParamType::Bool => {
            let boolean = value.as_bool().ok_or_else(|| {
                AppError::InvalidArgument("bool argument must be true/false".to_owned())
            })?;
            Ok(Token::Bool(boolean))
        }
        ParamType::String => {
            let text = value.as_str().ok_or_else(|| {
                AppError::InvalidArgument("string argument must be string".to_owned())
            })?;
            Ok(Token::String(text.to_owned()))
        }
        ParamType::Bytes => {
            let text = value.as_str().ok_or_else(|| {
                AppError::InvalidArgument("bytes argument must be hex string".to_owned())
            })?;
            let bytes = parse_hex_bytes(text, "bytes")?;
            Ok(Token::Bytes(bytes))
        }
        ParamType::FixedBytes(len) => {
            let text = value.as_str().ok_or_else(|| {
                AppError::InvalidArgument("fixed bytes argument must be hex string".to_owned())
            })?;
            let bytes = parse_hex_fixed(text, *len, "fixed bytes")?;
            Ok(Token::FixedBytes(bytes))
        }
        ParamType::Array(inner) => {
            let arr = value.as_array().ok_or_else(|| {
                AppError::InvalidArgument("array argument must be array".to_owned())
            })?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(parse_abi_token(inner, item)?);
            }
            Ok(Token::Array(out))
        }
        ParamType::FixedArray(inner, len) => {
            let arr = value.as_array().ok_or_else(|| {
                AppError::InvalidArgument("fixed array argument must be array".to_owned())
            })?;
            if arr.len() != *len {
                return Err(AppError::InvalidArgument(format!(
                    "fixed array expects {} items, got {}",
                    len,
                    arr.len()
                )));
            }
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(parse_abi_token(inner, item)?);
            }
            Ok(Token::FixedArray(out))
        }
        ParamType::Tuple(inner_types) => {
            let arr = value.as_array().ok_or_else(|| {
                AppError::InvalidArgument("tuple argument must be array".to_owned())
            })?;
            if arr.len() != inner_types.len() {
                return Err(AppError::InvalidArgument(format!(
                    "tuple expects {} items, got {}",
                    inner_types.len(),
                    arr.len()
                )));
            }
            let mut out = Vec::with_capacity(arr.len());
            for (item, kind) in arr.iter().zip(inner_types.iter()) {
                out.push(parse_abi_token(kind, item)?);
            }
            Ok(Token::Tuple(out))
        }
    }
}

fn parse_output_types(value: &Value) -> AppResult<Vec<ParamType>> {
    let arr = value
        .as_array()
        .ok_or_else(|| AppError::InvalidArgument("output_types must be array".to_owned()))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let text = item.as_str().ok_or_else(|| {
            AppError::InvalidArgument("output_types entries must be strings".to_owned())
        })?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidArgument(
                "invalid output type '': empty type string".to_owned(),
            ));
        }
        let kind = Reader::read(trimmed).map_err(|err| {
            AppError::InvalidArgument(format!("invalid output type '{trimmed}': {err}"))
        })?;
        out.push(kind);
    }
    Ok(out)
}

fn decode_abi_output(raw_result: &Value, output_types: &[ParamType]) -> AppResult<Value> {
    let raw_hex = raw_result.as_str().ok_or_else(|| {
        AppError::InvalidArgument("eth_call result must be hex string to decode output".to_owned())
    })?;
    let raw_bytes = parse_hex_bytes(raw_hex, "eth_call result")?;
    if output_types.is_empty() && !raw_bytes.is_empty() {
        return Err(AppError::InvalidArgument(
            "failed to decode ABI output bytes: expected empty bytes for empty output types"
                .to_owned(),
        ));
    }
    let tokens = ethabi::decode(output_types, &raw_bytes).map_err(|err| {
        AppError::InvalidArgument(format!("failed to decode ABI output bytes: {err}"))
    })?;

    let mut output = Vec::with_capacity(tokens.len());
    for token in tokens {
        output.push(token_to_json(&token));
    }
    Ok(Value::Array(output))
}

fn token_to_json(token: &Token) -> Value {
    match token {
        Token::Address(addr) => json!(format!("0x{}", hex::encode(addr.as_bytes()))),
        Token::FixedBytes(bytes) | Token::Bytes(bytes) => {
            json!(format!("0x{}", hex::encode(bytes)))
        }
        Token::Int(v) | Token::Uint(v) => json!(format!("{}", v)),
        Token::Bool(b) => json!(*b),
        Token::String(s) => json!(s),
        Token::Array(items) | Token::FixedArray(items) => {
            Value::Array(items.iter().map(token_to_json).collect())
        }
        Token::Tuple(items) => Value::Array(items.iter().map(token_to_json).collect()),
    }
}

fn parse_value_u256(value: &Value) -> AppResult<U256> {
    if let Some(text) = value.as_str() {
        if let Some(hex_part) = text.strip_prefix("0x") {
            if hex_part.is_empty() || !hex_part.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(AppError::InvalidArgument(format!(
                    "invalid hex integer '{text}': non-hex characters"
                )));
            }
            U256::from_str_radix(hex_part, 16).map_err(|err| {
                AppError::InvalidArgument(format!("invalid hex integer '{text}': {err}"))
            })
        } else {
            if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err(AppError::InvalidArgument(format!(
                    "invalid decimal integer '{text}': non-decimal characters"
                )));
            }
            U256::from_dec_str(text).map_err(|err| {
                AppError::InvalidArgument(format!("invalid decimal integer '{text}': {err}"))
            })
        }
    } else if let Some(number) = value.as_u64() {
        Ok(U256::from(number))
    } else {
        Err(AppError::InvalidArgument(
            "integer argument must be string or integer".to_owned(),
        ))
    }
}

fn parse_hex_bytes(input: &str, field: &str) -> AppResult<Vec<u8>> {
    let normalized = input.strip_prefix("0x").unwrap_or(input);
    hex::decode(normalized).map_err(|err| {
        AppError::InvalidArgument(format!("{field} must be hex; decode failed: {err}"))
    })
}

fn stringify_json(value: &Value, field: &str) -> AppResult<String> {
    if let Some(text) = value.as_str() {
        Ok(text.to_owned())
    } else {
        Err(AppError::Rpc(format!("{field} was not a string")))
    }
}

fn parse_hex_u64(value: &str) -> AppResult<u64> {
    let normalized = value.strip_prefix("0x").unwrap_or(value);
    if normalized.is_empty() || !normalized.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::Rpc(format!(
            "failed to parse hex u64 value '{value}' from RPC response: non-hex characters"
        )));
    }
    u64::from_str_radix(normalized, 16).map_err(|err| {
        AppError::Rpc(format!(
            "failed to parse hex u64 value '{value}' from RPC response: {err}"
        ))
    })
}

fn hex_eq(left: &str, right: &str) -> bool {
    let left_norm = left.strip_prefix("0x").unwrap_or(left).to_ascii_lowercase();
    let right_norm = right
        .strip_prefix("0x")
        .unwrap_or(right)
        .to_ascii_lowercase();
    left_norm == right_norm
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Auth, DeploymentPolicy, Limits, Policy};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn context_with_policy(policy: Policy) -> ToolContext {
        ToolContext {
            config: Config {
                limits: Limits::default(),
                policy,
                auth: Auth::default(),
                deployment: DeploymentPolicy::default(),
                networks: BTreeMap::new(),
            },
        }
    }

    fn context_with_sora_network(pallet_index: Option<u8>) -> ToolContext {
        let mut networks = BTreeMap::new();
        networks.insert(
            "sora_local".to_owned(),
            NetworkProfile {
                kind: NetworkKind::Sora,
                rpc_url: "http://127.0.0.1:9933".to_owned(),
                ws_url: None,
                chain_id: None,
                genesis_hash: None,
                ss58_prefix: None,
                sccp_pallet_index: pallet_index,
                block_number_bytes: 4,
                router_address: None,
                notes: None,
            },
        );

        ToolContext {
            config: Config {
                limits: Limits::default(),
                policy: Policy::default(),
                auth: Auth::default(),
                deployment: DeploymentPolicy::default(),
                networks,
            },
        }
    }

    #[test]
    fn high_risk_tool_classifier_matches_submit_surface() {
        assert!(is_high_risk_tool("sora_sccp_submit_signed_extrinsic"));
        assert!(is_high_risk_tool("evm_sccp_submit_signed_tx"));
        assert!(is_high_risk_tool("sol_sccp_submit_signed_transaction"));
        assert!(is_high_risk_tool("ton_sccp_submit_signed_message"));
        assert!(!is_high_risk_tool("sccp_preflight_activation"));
        assert!(!is_high_risk_tool("sccp_list_networks"));
    }

    #[test]
    fn audit_network_hint_extracts_string_or_defaults_to_unknown() {
        assert_eq!(
            audit_network_hint(&json!({"network":"sora_local"})),
            "sora_local"
        );
        assert_eq!(audit_network_hint(&json!({"network":7})), "unknown");
        assert_eq!(audit_network_hint(&json!({})), "unknown");
    }

    #[test]
    fn required_string_rejects_missing_and_non_string_fields() {
        let missing =
            required_string(&json!({}), "network").expect_err("missing field should fail");
        assert!(
            missing
                .to_string()
                .contains("missing string field 'network'"),
            "unexpected error: {missing}"
        );

        let non_string = required_string(&json!({ "network": 7 }), "network")
            .expect_err("non-string field should fail");
        assert!(
            non_string
                .to_string()
                .contains("missing string field 'network'"),
            "unexpected error: {non_string}"
        );
    }

    #[test]
    fn optional_string_accepts_null_and_rejects_non_string() {
        let none = optional_string(&json!({ "nonce": null }), "nonce")
            .expect("null optional string should map to None");
        assert!(none.is_none());

        let text = optional_string(&json!({ "nonce": "0x01" }), "nonce")
            .expect("string optional field should parse");
        assert_eq!(text.as_deref(), Some("0x01"));

        let non_string = optional_string(&json!({ "nonce": 1 }), "nonce")
            .expect_err("non-string optional field should fail");
        assert!(
            non_string
                .to_string()
                .contains("field 'nonce' must be a string when provided"),
            "unexpected error: {non_string}"
        );
    }

    #[test]
    fn required_u32_rejects_missing_non_integer_and_overflow_fields() {
        let missing =
            required_u32(&json!({}), "domain_id").expect_err("missing integer field should fail");
        assert!(
            missing
                .to_string()
                .contains("missing integer field 'domain_id'"),
            "unexpected error: {missing}"
        );

        let non_integer = required_u32(&json!({ "domain_id": "1" }), "domain_id")
            .expect_err("string domain id should fail");
        assert!(
            non_integer
                .to_string()
                .contains("missing integer field 'domain_id'"),
            "unexpected error: {non_integer}"
        );

        let overflow = required_u32(&json!({ "domain_id": 4_294_967_296u64 }), "domain_id")
            .expect_err("u32 overflow should fail");
        assert!(
            overflow
                .to_string()
                .contains("field 'domain_id' does not fit u32"),
            "unexpected error: {overflow}"
        );

        let float_value = required_u32(&json!({ "domain_id": 1.5 }), "domain_id")
            .expect_err("float field should fail");
        assert!(
            float_value
                .to_string()
                .contains("missing integer field 'domain_id'"),
            "unexpected error: {float_value}"
        );
    }

    #[test]
    fn domain_ids_for_preflight_defaults_to_core_domains() {
        let domains = domain_ids_for_preflight(&json!({}))
            .expect("missing domain_ids should use default core domains");
        assert_eq!(domains, SCCP_CORE_REMOTE_DOMAINS.to_vec());
    }

    #[test]
    fn domain_ids_for_preflight_validates_and_deduplicates() {
        let domains = domain_ids_for_preflight(&json!({
            "domain_ids": [SCCP_DOMAIN_ETH, SCCP_DOMAIN_SOL, SCCP_DOMAIN_ETH, SCCP_DOMAIN_TRON]
        }))
        .expect("domain_ids should parse and dedupe");
        assert_eq!(
            domains,
            vec![SCCP_DOMAIN_ETH, SCCP_DOMAIN_SOL, SCCP_DOMAIN_TRON]
        );
    }

    #[test]
    fn domain_ids_for_preflight_rejects_invalid_payloads() {
        let not_array = domain_ids_for_preflight(&json!({"domain_ids": "1"}))
            .expect_err("non-array domain_ids should fail");
        assert!(
            not_array
                .to_string()
                .contains("field 'domain_ids' must be an array"),
            "unexpected error: {not_array}"
        );

        let empty = domain_ids_for_preflight(&json!({"domain_ids": []}))
            .expect_err("empty domain_ids should fail");
        assert!(
            empty.to_string().contains("must not be empty"),
            "unexpected error: {empty}"
        );

        let unsupported = domain_ids_for_preflight(&json!({"domain_ids": [0]}))
            .expect_err("SORA domain should be rejected for activation preflight");
        assert!(
            unsupported.to_string().contains("unsupported"),
            "unexpected error: {unsupported}"
        );
    }

    #[test]
    fn decoded_hex_len_bytes_decodes_lengths_and_rejects_invalid_hex() {
        assert_eq!(
            decoded_hex_len_bytes(Some("0x0011")).expect("valid hex should decode"),
            Some(2)
        );
        assert_eq!(
            decoded_hex_len_bytes(None).expect("missing value should map to None"),
            None
        );

        let err = decoded_hex_len_bytes(Some("0xzz")).expect_err("invalid hex should fail");
        assert!(
            err.to_string().contains("failed to decode hex value"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn ensure_ton_read_method_allows_get_prefix_and_run_get_method() {
        ensure_ton_read_method("getMasterchainInfo")
            .expect("get* methods should be allowed for ton read tool");
        ensure_ton_read_method("GETtransactions")
            .expect("case-insensitive get* methods should be allowed");
        ensure_ton_read_method("runGetMethod")
            .expect("runGetMethod should be allowed for read-only contract calls");
    }

    #[test]
    fn ensure_ton_read_method_rejects_mutating_or_invalid_method_names() {
        let send_err = ensure_ton_read_method("sendBoc")
            .expect_err("send* methods should be rejected on ton read tool");
        assert!(
            send_err.to_string().contains("read-only methods"),
            "unexpected error: {send_err}"
        );

        let submit_err = ensure_ton_read_method("submitTx")
            .expect_err("submit* methods should be rejected on ton read tool");
        assert!(
            submit_err.to_string().contains("read-only methods"),
            "unexpected error: {submit_err}"
        );

        let whitespace_err = ensure_ton_read_method(" getMasterchainInfo")
            .expect_err("leading/trailing whitespace should fail closed");
        assert!(
            whitespace_err.to_string().contains("must not include"),
            "unexpected error: {whitespace_err}"
        );
    }

    #[test]
    fn parse_hex_u64_accepts_prefixed_and_unprefixed() {
        assert_eq!(parse_hex_u64("0xff").expect("0xff should parse"), 255);
        assert_eq!(parse_hex_u64("ff").expect("ff should parse"), 255);
        assert_eq!(parse_hex_u64("0xFF").expect("0xFF should parse"), 255);
    }

    #[test]
    fn parse_hex_u64_rejects_invalid_input() {
        let error = parse_hex_u64("0xzz").expect_err("invalid hex must fail");
        assert!(
            error.to_string().contains("failed to parse hex u64"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_u64_rejects_overflow() {
        let error = parse_hex_u64("0x10000000000000000").expect_err("overflow must fail");
        assert!(
            error.to_string().contains("failed to parse hex u64"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_u64_rejects_negative_numbers() {
        let error = parse_hex_u64("-1").expect_err("negative values must fail");
        assert!(
            error.to_string().contains("failed to parse hex u64"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_u64_rejects_uppercase_hex_prefix() {
        let error = parse_hex_u64("0Xff").expect_err("uppercase 0X prefix should fail closed");
        assert!(
            error.to_string().contains("failed to parse hex u64"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_u64_rejects_empty_input() {
        let error = parse_hex_u64("").expect_err("empty string must fail");
        assert!(
            error.to_string().contains("failed to parse hex u64"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_u64_rejects_whitespace_wrapped_input() {
        let error = parse_hex_u64(" 0xff ").expect_err("whitespace-wrapped input must fail");
        assert!(
            error.to_string().contains("failed to parse hex u64"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_u64_rejects_plus_prefixed_input() {
        let error = parse_hex_u64("+ff").expect_err("plus-prefixed input must fail");
        assert!(
            error.to_string().contains("failed to parse hex u64"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hex_eq_is_case_insensitive_and_prefix_agnostic() {
        assert!(hex_eq("0xAbCd", "abcd"));
        assert!(hex_eq("abcd", "0xABCD"));
        assert!(!hex_eq("0x01", "0x02"));
    }

    #[test]
    fn encode_abi_call_builds_calldata() {
        let args = json!([1, true]);
        let encoded = encode_abi_call("setOutboundDomainPaused(uint32,bool)", &args)
            .expect("calldata should encode");
        assert!(encoded.starts_with("0x"), "must be hex encoded");
        assert_eq!(encoded.len(), 2 + 8 + 64 + 64); // selector + two ABI words
    }

    #[test]
    fn encode_abi_call_rejects_arg_count_mismatch() {
        let args = json!([1]);
        let error = encode_abi_call("setOutboundDomainPaused(uint32,bool)", &args)
            .expect_err("arg mismatch should fail");
        assert!(
            error.to_string().contains("argument count mismatch"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn encode_abi_call_supports_zero_argument_signatures() {
        let encoded =
            encode_abi_call("sync()", &json!([])).expect("zero-arg ABI signature should encode");
        assert!(encoded.starts_with("0x"), "must be hex encoded");
        assert_eq!(encoded.len(), 2 + 8, "selector-only calldata expected");
    }

    #[test]
    fn encode_abi_call_rejects_invalid_function_name_in_signature() {
        let args = json!([1, true]);
        let error = encode_abi_call("set Outbound(uint32,bool)", &args)
            .expect_err("invalid function name should fail");
        assert!(
            error.to_string().contains("invalid function name"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_empty_function_name() {
        let error = parse_signature("(uint32)").expect_err("empty function name must fail");
        assert!(
            error
                .to_string()
                .contains("function name in signature cannot be empty"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_invalid_function_name_tokens() {
        let with_space = parse_signature("set Outbound(uint32)")
            .expect_err("function name with whitespace must fail");
        assert!(
            with_space.to_string().contains("invalid function name"),
            "unexpected error: {with_space}"
        );

        let starts_with_digit = parse_signature("1foo(uint32)")
            .expect_err("function name starting with digit must fail");
        assert!(
            starts_with_digit
                .to_string()
                .contains("invalid function name"),
            "unexpected error: {starts_with_digit}"
        );

        let punctuation = parse_signature("foo-bar(uint32)")
            .expect_err("function name with punctuation must fail");
        assert!(
            punctuation.to_string().contains("invalid function name"),
            "unexpected error: {punctuation}"
        );

        let dotted =
            parse_signature("foo.bar(uint32)").expect_err("function name with dot must fail");
        assert!(
            dotted.to_string().contains("invalid function name"),
            "unexpected error: {dotted}"
        );

        let unicode =
            parse_signature("fóo(uint32)").expect_err("non-ascii function name must fail");
        assert!(
            unicode.to_string().contains("invalid function name"),
            "unexpected error: {unicode}"
        );
    }

    #[test]
    fn parse_signature_accepts_valid_function_name_with_underscore_and_digits() {
        let (name, params) =
            parse_signature("_foo1(uint32)").expect("valid function name should parse");
        assert_eq!(name, "_foo1");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn parse_signature_accepts_whitespace_around_name_and_types() {
        let (name, params) = parse_signature("  setOutboundDomainPaused ( uint32 , bool ) ")
            .expect("signature with surrounding whitespace should parse");
        assert_eq!(name, "setOutboundDomainPaused");
        assert_eq!(params, vec![ParamType::Uint(32), ParamType::Bool]);
    }

    #[test]
    fn parse_signature_rejects_missing_parenthesis() {
        let error = parse_signature("setOutboundDomainPaused(uint32,bool")
            .expect_err("signature without closing parenthesis must fail");
        assert!(
            error.to_string().contains("missing ')'"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_missing_open_parenthesis() {
        let error = parse_signature("setOutboundDomainPauseduint32,bool)")
            .expect_err("signature without opening parenthesis must fail");
        assert!(
            error.to_string().contains("missing '('"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_trailing_characters() {
        let error = parse_signature("setOutboundDomainPaused(uint32,bool)extra")
            .expect_err("signature with trailing suffix must fail");
        assert!(
            error.to_string().contains("trailing characters"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_close_before_open() {
        let error = parse_signature("setOutboundDomainPaused)uint32(bool")
            .expect_err("signature with ')' before '(' must fail");
        assert!(
            error.to_string().contains("invalid signature"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_accepts_nested_tuple_and_array_types() {
        let (name, params) =
            parse_signature("f((uint32,bool),bytes32[])").expect("signature should parse");
        assert_eq!(name, "f");
        assert_eq!(params.len(), 2);
        assert!(
            matches!(params[0], ParamType::Tuple(_)),
            "first param should be tuple"
        );
        assert!(
            matches!(params[1], ParamType::Array(_)),
            "second param should be array"
        );
    }

    #[test]
    fn parse_signature_rejects_empty_parameter_entries() {
        let error = parse_signature("f(uint32,,bool)")
            .expect_err("empty parameter entries must fail closed");
        assert!(
            error.to_string().contains("empty parameter type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_leading_empty_parameter_entry() {
        let error = parse_signature("f(,bool)")
            .expect_err("leading empty parameter entry must fail closed");
        assert!(
            error.to_string().contains("empty parameter type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_trailing_empty_parameter_entry() {
        let error = parse_signature("f(uint32,)")
            .expect_err("trailing empty parameter entry must fail closed");
        assert!(
            error.to_string().contains("empty parameter type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_whitespace_only_parameter_entry() {
        let error = parse_signature("f(uint32,   ,bool)")
            .expect_err("whitespace-only parameter entries must fail closed");
        assert!(
            error.to_string().contains("empty parameter type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_signature_rejects_nested_tuple_with_empty_entry() {
        let error = parse_signature("f((uint32,),bool)")
            .expect_err("nested tuple with empty entry must fail closed");
        assert!(
            error.to_string().contains("empty parameter type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn ensure_hex_string_rejects_missing_prefix_and_invalid_hex() {
        let missing_prefix =
            ensure_hex_string("abcd", "field").expect_err("missing prefix must fail");
        assert!(
            missing_prefix.to_string().contains("0x-prefixed"),
            "unexpected error: {missing_prefix}"
        );

        let invalid_hex = ensure_hex_string("0xzz", "field").expect_err("invalid hex must fail");
        assert!(
            invalid_hex.to_string().contains("valid hex"),
            "unexpected error: {invalid_hex}"
        );
    }

    #[test]
    fn ensure_hex_string_rejects_uppercase_hex_prefix() {
        let error =
            ensure_hex_string("0X11", "field").expect_err("uppercase 0X prefix should fail");
        assert!(
            error.to_string().contains("0x-prefixed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn ensure_hex_or_decimal_rejects_invalid_decimal_string() {
        let error = ensure_hex_or_decimal("12x", "nonce").expect_err("invalid decimal must fail");
        assert!(
            error
                .to_string()
                .contains("must be decimal or 0x hex integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn ensure_hex_or_decimal_rejects_uppercase_hex_prefix() {
        let error = ensure_hex_or_decimal("0X10", "nonce")
            .expect_err("uppercase 0X prefix should fail closed");
        assert!(
            error
                .to_string()
                .contains("must be decimal or 0x hex integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn ensure_hex_or_decimal_rejects_whitespace_wrapped_decimal() {
        let error = ensure_hex_or_decimal(" 10 ", "nonce")
            .expect_err("whitespace-wrapped decimal should fail closed");
        assert!(
            error
                .to_string()
                .contains("must be decimal or 0x hex integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn ensure_hex_or_decimal_accepts_valid_decimal_and_hex_inputs() {
        ensure_hex_or_decimal("10", "nonce").expect("plain decimal should be accepted");
        ensure_hex_or_decimal("0x10", "nonce").expect("prefixed lowercase hex should be accepted");
        ensure_hex_or_decimal("0xABCD", "nonce")
            .expect("prefixed uppercase hex digits should be accepted");
    }

    #[test]
    fn parse_output_types_rejects_non_string_entries() {
        let error = parse_output_types(&json!(["uint256", 7]))
            .expect_err("non-string output type entries must fail");
        assert!(
            error
                .to_string()
                .contains("output_types entries must be strings"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_output_types_rejects_non_array() {
        let error =
            parse_output_types(&json!("uint256")).expect_err("non-array output_types must fail");
        assert!(
            error.to_string().contains("output_types must be array"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_output_types_rejects_invalid_type_string() {
        let error =
            parse_output_types(&json!(["uint256["])).expect_err("invalid ABI type names must fail");
        assert!(
            error.to_string().contains("invalid output type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_output_types_rejects_whitespace_only_entry() {
        let error = parse_output_types(&json!(["   "]))
            .expect_err("whitespace-only output type must fail closed");
        assert!(
            error.to_string().contains("invalid output type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_output_types_accepts_nested_tuple_and_array_types() {
        let kinds = parse_output_types(&json!(["(uint256,bool)", "bytes32[]"]))
            .expect("nested output types should parse");
        assert_eq!(kinds.len(), 2);
        assert!(matches!(kinds[0], ParamType::Tuple(_)));
        assert!(matches!(kinds[1], ParamType::Array(_)));
    }

    #[test]
    fn parse_output_types_accepts_empty_array() {
        let kinds = parse_output_types(&json!([])).expect("empty output type array should parse");
        assert!(kinds.is_empty(), "expected no output types");
    }

    #[test]
    fn parse_output_types_accepts_whitespace_wrapped_valid_entries() {
        let kinds = parse_output_types(&json!([" uint256 ", " (bool,bytes32[]) "]))
            .expect("whitespace-wrapped valid output types should parse");
        assert_eq!(kinds.len(), 2);
        assert!(matches!(kinds[0], ParamType::Uint(256)));
        assert!(matches!(kinds[1], ParamType::Tuple(_)));
    }

    #[test]
    fn parse_abi_token_parses_nested_tuple_and_array_value() {
        let kind = ParamType::Tuple(vec![
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Bool)),
        ]);
        let token = parse_abi_token(&kind, &json!(["7", [true, false]]))
            .expect("nested tuple+array argument should parse");
        assert_eq!(token_to_json(&token), json!(["7", [true, false]]));
    }

    #[test]
    fn parse_abi_token_rejects_non_string_address_argument() {
        let error = parse_abi_token(&ParamType::Address, &json!(7))
            .expect_err("non-string address must fail");
        assert!(
            error
                .to_string()
                .contains("address argument must be string"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_abi_token_rejects_wrong_length_address_argument() {
        let error = parse_abi_token(&ParamType::Address, &json!("0x11"))
            .expect_err("wrong-length address must fail");
        assert!(
            error.to_string().contains("address must be 20 bytes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_abi_token_rejects_negative_signed_integer_argument() {
        let error = parse_abi_token(&ParamType::Int(256), &json!("-1"))
            .expect_err("negative int argument should fail closed");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_abi_token_rejects_fixed_array_length_mismatch() {
        let kind = ParamType::FixedArray(Box::new(ParamType::Uint(256)), 2);
        let error = parse_abi_token(&kind, &json!([1u64]))
            .expect_err("fixed array length mismatch must fail");
        assert!(
            error.to_string().contains("fixed array expects 2 items"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_abi_token_rejects_tuple_length_mismatch() {
        let kind = ParamType::Tuple(vec![ParamType::Uint(256), ParamType::Bool]);
        let error =
            parse_abi_token(&kind, &json!([1u64])).expect_err("tuple length mismatch must fail");
        assert!(
            error.to_string().contains("tuple expects 2 items"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_negative_decimal_strings() {
        let error =
            parse_value_u256(&json!("-1")).expect_err("negative decimal input must fail for u256");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_hex_overflow() {
        let overflow = format!("0x1{}", "0".repeat(64));
        let error =
            parse_value_u256(&json!(overflow)).expect_err("value larger than u256 must fail");
        assert!(
            error.to_string().contains("invalid hex integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_accepts_max_decimal_and_hex_strings() {
        let max_decimal =
            "115792089237316195423570985008687907853269984665640564039457584007913129639935";
        let max_hex = format!("0x{}", "f".repeat(64));
        assert_eq!(
            parse_value_u256(&json!(max_decimal)).expect("max decimal string should parse"),
            U256::MAX
        );
        assert_eq!(
            parse_value_u256(&json!(max_hex)).expect("max hex string should parse"),
            U256::MAX
        );
        assert_eq!(
            parse_value_u256(&json!("0xABCD")).expect("uppercase hex digits should parse"),
            U256::from(0xABCDu64)
        );
    }

    #[test]
    fn parse_value_u256_rejects_decimal_overflow() {
        let overflow_decimal =
            "115792089237316195423570985008687907853269984665640564039457584007913129639936";
        let error = parse_value_u256(&json!(overflow_decimal))
            .expect_err("decimal larger than u256::MAX must fail");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_negative_json_number() {
        let error =
            parse_value_u256(&json!(-1)).expect_err("negative json numbers must fail for u256");
        assert!(
            error
                .to_string()
                .contains("integer argument must be string or integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_float_json_number() {
        let error =
            parse_value_u256(&json!(1.5)).expect_err("floating-point json numbers must fail");
        assert!(
            error
                .to_string()
                .contains("integer argument must be string or integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_boolean_json_values() {
        let error =
            parse_value_u256(&json!(true)).expect_err("boolean json values must fail for u256");
        assert!(
            error
                .to_string()
                .contains("integer argument must be string or integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_plus_prefixed_decimal_string() {
        let error = parse_value_u256(&json!("+1"))
            .expect_err("plus-prefixed decimal string must fail for u256");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_plus_prefixed_hex_string() {
        let error = parse_value_u256(&json!("0x+1"))
            .expect_err("plus-prefixed hex string must fail for u256");
        assert!(
            error.to_string().contains("invalid hex integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_uppercase_hex_prefix() {
        let error = parse_value_u256(&json!("0Xff"))
            .expect_err("uppercase 0X prefix must fail closed for u256");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_empty_string() {
        let error = parse_value_u256(&json!("")).expect_err("empty string must fail for u256");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_scientific_notation_string() {
        let error = parse_value_u256(&json!("1e3"))
            .expect_err("scientific-notation strings must fail for u256");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_whitespace_wrapped_decimal_string() {
        let error = parse_value_u256(&json!(" 1 "))
            .expect_err("whitespace-wrapped decimal string must fail for u256");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_value_u256_rejects_whitespace_wrapped_hex_string() {
        let error = parse_value_u256(&json!(" 0xff "))
            .expect_err("whitespace-wrapped hex string must fail for u256");
        assert!(
            error.to_string().contains("invalid decimal integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_bytes_rejects_odd_length_hex() {
        let error = parse_hex_bytes("0xabc", "bytes").expect_err("odd-length hex bytes must fail");
        assert!(
            error.to_string().contains("decode failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_bytes_rejects_plus_prefixed_hex() {
        let error =
            parse_hex_bytes("0x+1", "bytes").expect_err("plus-prefixed hex bytes must fail");
        assert!(
            error.to_string().contains("decode failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_bytes_rejects_non_hex_digits() {
        let error = parse_hex_bytes("0x1z", "bytes").expect_err("non-hex bytes must fail");
        assert!(
            error.to_string().contains("decode failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_bytes_accepts_unprefixed_hex() {
        let bytes =
            parse_hex_bytes("deadbeef", "bytes").expect("unprefixed hex bytes should parse");
        assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn parse_hex_bytes_rejects_uppercase_prefix() {
        let error =
            parse_hex_bytes("0Xff", "bytes").expect_err("uppercase 0X prefix must fail closed");
        assert!(
            error.to_string().contains("decode failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_hex_bytes_rejects_whitespace_wrapped_hex() {
        let error =
            parse_hex_bytes(" 0xff ", "bytes").expect_err("whitespace-wrapped hex bytes must fail");
        assert!(
            error.to_string().contains("decode failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn decode_abi_output_decodes_bool_and_uint_values() {
        let raw = format!(
            "0x{}",
            hex::encode(ethabi::encode(&[
                Token::Bool(true),
                Token::Uint(U256::from(7u64)),
            ]))
        );
        let decoded = decode_abi_output(&json!(raw), &[ParamType::Bool, ParamType::Uint(256)])
            .expect("ABI output should decode");
        assert_eq!(decoded, json!([true, "7"]));
    }

    #[test]
    fn decode_abi_output_decodes_tuple_with_array_items() {
        let raw = format!(
            "0x{}",
            hex::encode(ethabi::encode(&[Token::Tuple(vec![
                Token::Uint(U256::from(7u64)),
                Token::Array(vec![Token::Bool(true), Token::Bool(false)]),
            ]),]))
        );
        let decoded = decode_abi_output(
            &json!(raw),
            &[ParamType::Tuple(vec![
                ParamType::Uint(256),
                ParamType::Array(Box::new(ParamType::Bool)),
            ])],
        )
        .expect("tuple+array ABI output should decode");
        assert_eq!(decoded, json!([["7", [true, false]]]));
    }

    #[test]
    fn decode_abi_output_rejects_invalid_hex() {
        let error = decode_abi_output(&json!("0xzz"), &[ParamType::Bool])
            .expect_err("invalid hex output must fail");
        assert!(
            error.to_string().contains("decode failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn decode_abi_output_rejects_uppercase_hex_prefix() {
        let error = decode_abi_output(&json!("0X01"), &[ParamType::Bool])
            .expect_err("uppercase hex prefix must fail closed");
        assert!(
            error.to_string().contains("decode failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn decode_abi_output_rejects_whitespace_wrapped_hex() {
        let error = decode_abi_output(&json!(" 0x01 "), &[ParamType::Bool])
            .expect_err("whitespace-wrapped hex output must fail closed");
        assert!(
            error.to_string().contains("decode failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn decode_abi_output_rejects_truncated_bytes() {
        let error = decode_abi_output(&json!("0x01"), &[ParamType::Bool])
            .expect_err("truncated ABI output bytes must fail");
        assert!(
            error
                .to_string()
                .contains("failed to decode ABI output bytes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn decode_abi_output_rejects_type_length_mismatch() {
        let raw = format!("0x{}", hex::encode(ethabi::encode(&[Token::Bool(true)])));
        let error = decode_abi_output(&json!(raw), &[ParamType::Bool, ParamType::Uint(256)])
            .expect_err("ABI type/result length mismatch must fail");
        assert!(
            error
                .to_string()
                .contains("failed to decode ABI output bytes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn decode_abi_output_accepts_empty_bytes_for_empty_output_types() {
        let decoded = decode_abi_output(&json!("0x"), &[])
            .expect("empty output should decode for empty output types");
        assert_eq!(decoded, json!([]));
    }

    #[test]
    fn decode_abi_output_rejects_non_empty_bytes_for_empty_output_types() {
        let error = decode_abi_output(&json!("0x01"), &[])
            .expect_err("non-empty output bytes must fail when no output types are declared");
        assert!(
            error
                .to_string()
                .contains("failed to decode ABI output bytes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn decode_abi_output_rejects_non_string_result() {
        let error = decode_abi_output(&json!(7), &[ParamType::Bool])
            .expect_err("non-string eth_call result must fail");
        assert!(
            error
                .to_string()
                .contains("eth_call result must be hex string"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn encode_abi_call_rejects_non_array_args() {
        let error = encode_abi_call(
            "setOutboundDomainPaused(uint32,bool)",
            &json!({"domain_id": 1, "paused": true}),
        )
        .expect_err("args must be array");
        assert!(
            error.to_string().contains("'args' must be an array"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn encode_abi_call_accepts_whitespace_padded_signature() {
        let args = json!([1, true]);
        let encoded = encode_abi_call("  setOutboundDomainPaused ( uint32 , bool )  ", &args)
            .expect("whitespace-padded signature should encode");
        assert!(encoded.starts_with("0x"), "must be hex encoded");
        assert_eq!(encoded.len(), 2 + 8 + 64 + 64);
    }

    #[test]
    fn dispatch_rejects_denied_tool() {
        let ctx = context_with_policy(Policy {
            allow_tools: vec![],
            deny_tools: vec!["sccp_list_networks".to_owned()],
        });

        let error = dispatch(&ctx, "sccp_list_networks", &json!({}))
            .expect_err("denied tools must not execute");
        assert!(
            matches!(error, AppError::ToolDenied(_)),
            "unexpected error variant: {error}"
        );
    }

    #[test]
    fn dispatch_rejects_tool_not_present_in_allow_list() {
        let ctx = context_with_policy(Policy {
            allow_tools: vec!["sccp_health".to_owned()],
            deny_tools: vec![],
        });

        let error = dispatch(&ctx, "sccp_list_networks", &json!({}))
            .expect_err("allow-list should block non-listed tool names");
        assert!(
            matches!(error, AppError::ToolDenied(_)),
            "unexpected error variant: {error}"
        );
    }

    #[test]
    fn dispatch_rejects_unknown_tool_name() {
        let ctx = context_with_policy(Policy {
            allow_tools: vec!["sccp_unknown_tool".to_owned()],
            deny_tools: vec![],
        });
        let error = dispatch(&ctx, "sccp_unknown_tool", &json!({}))
            .expect_err("unknown tool names must fail");
        assert!(
            error.to_string().contains("unknown tool name"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn payload_tools_reject_payloads_with_unknown_fields() {
        let ctx = context_with_policy(Policy::default());
        let payload = json!({
            "version": 1,
            "source_domain": 0,
            "dest_domain": 1,
            "nonce": 7,
            "sora_asset_id": "0x1111111111111111111111111111111111111111111111111111111111111111",
            "amount": "1",
            "recipient": "0x0000000000000000000000002222222222222222222222222222222222222222",
            "unexpected": "x"
        });

        let validate_err = dispatch(
            &ctx,
            "sccp_validate_payload",
            &json!({ "payload": payload.clone() }),
        )
        .expect_err("payload validator should reject unknown payload fields");
        assert!(
            validate_err.to_string().contains("invalid burn payload"),
            "unexpected error: {validate_err}"
        );

        let message_id_err = dispatch(&ctx, "sccp_get_message_id", &json!({ "payload": payload }))
            .expect_err("message-id tool should reject unknown payload fields");
        assert!(
            message_id_err.to_string().contains("invalid burn payload"),
            "unexpected error: {message_id_err}"
        );
    }

    #[test]
    fn payload_tools_reject_missing_payload_argument() {
        let ctx = context_with_policy(Policy::default());

        let validate_err = dispatch(&ctx, "sccp_validate_payload", &json!({}))
            .expect_err("missing payload argument must fail");
        assert!(
            validate_err.to_string().contains("missing field 'payload'"),
            "unexpected error: {validate_err}"
        );

        let message_id_err = dispatch(&ctx, "sccp_get_message_id", &json!({}))
            .expect_err("missing payload argument must fail");
        assert!(
            message_id_err
                .to_string()
                .contains("missing field 'payload'"),
            "unexpected error: {message_id_err}"
        );
    }

    #[test]
    fn payload_tools_reject_non_object_payload_argument() {
        let ctx = context_with_policy(Policy::default());

        let validate_err = dispatch(&ctx, "sccp_validate_payload", &json!({"payload": "bad"}))
            .expect_err("non-object payload argument must fail");
        assert!(
            validate_err.to_string().contains("invalid burn payload"),
            "unexpected error: {validate_err}"
        );

        let message_id_err = dispatch(&ctx, "sccp_get_message_id", &json!({"payload": "bad"}))
            .expect_err("non-object payload argument must fail");
        assert!(
            message_id_err.to_string().contains("invalid burn payload"),
            "unexpected error: {message_id_err}"
        );
    }

    #[test]
    fn payload_tools_reject_whitespace_wrapped_amount_strings() {
        let ctx = context_with_policy(Policy::default());
        let payload = json!({
            "version": 1,
            "source_domain": 0,
            "dest_domain": 1,
            "nonce": 7,
            "sora_asset_id": "0x1111111111111111111111111111111111111111111111111111111111111111",
            "amount": " 1 ",
            "recipient": "0x0000000000000000000000002222222222222222222222222222222222222222",
        });

        let validate_err = dispatch(
            &ctx,
            "sccp_validate_payload",
            &json!({ "payload": payload.clone() }),
        )
        .expect_err("whitespace-wrapped decimal amount should fail closed");
        assert!(
            validate_err.to_string().contains("invalid amount decimal"),
            "unexpected error: {validate_err}"
        );

        let message_id_err = dispatch(&ctx, "sccp_get_message_id", &json!({ "payload": payload }))
            .expect_err("whitespace-wrapped decimal amount should fail closed");
        assert!(
            message_id_err
                .to_string()
                .contains("invalid amount decimal"),
            "unexpected error: {message_id_err}"
        );
    }

    #[test]
    fn attester_quorum_tool_rejects_unsupported_proof_version() {
        let ctx = context_with_policy(Policy::default());
        let error = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 2,
                "signatures": [],
            }),
        )
        .expect_err("version != 1 must fail");

        assert!(
            error
                .to_string()
                .contains("attester quorum proof version must currently be 1"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_quorum_tool_rejects_non_integer_version() {
        let ctx = context_with_policy(Policy::default());
        let error = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": "1",
                "signatures": []
            }),
        )
        .expect_err("string version must fail");
        assert!(
            error
                .to_string()
                .contains("field 'version' must be integer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_quorum_tool_rejects_missing_signatures_field() {
        let ctx = context_with_policy(Policy::default());
        let error = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 1
            }),
        )
        .expect_err("missing signatures must fail");
        assert!(
            error
                .to_string()
                .contains("missing array field 'signatures'"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_quorum_tool_rejects_non_array_signatures_field() {
        let ctx = context_with_policy(Policy::default());
        let error = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 1,
                "signatures": "0x"
            }),
        )
        .expect_err("non-array signatures must fail");
        assert!(
            error
                .to_string()
                .contains("missing array field 'signatures'"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_quorum_tool_rejects_non_string_signature_entries() {
        let ctx = context_with_policy(Policy::default());
        let error = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 1,
                "signatures": [7]
            }),
        )
        .expect_err("non-string signatures must fail");
        assert!(
            error
                .to_string()
                .contains("signatures[0] must be hex string"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_quorum_tool_rejects_wrong_signature_length() {
        let ctx = context_with_policy(Policy::default());
        let error = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 1,
                "signatures": ["0x11"]
            }),
        )
        .expect_err("non-65-byte signature should fail");
        assert!(
            error.to_string().contains("signatures[0] must be 65 bytes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_quorum_tool_rejects_uppercase_hex_prefix_signature() {
        let ctx = context_with_policy(Policy::default());
        let error = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 1,
                "signatures": [format!("0X{}", "11".repeat(65))]
            }),
        )
        .expect_err("uppercase 0X signatures should fail closed");
        assert!(
            error.to_string().contains("signatures[0] must be hex"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_quorum_tool_rejects_version_out_of_u8_range() {
        let ctx = context_with_policy(Policy::default());
        let error = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 256,
                "signatures": [format!("0x{}", "11".repeat(65))]
            }),
        )
        .expect_err("version > u8::MAX must fail");
        assert!(
            error
                .to_string()
                .contains("field 'version' does not fit u8"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn attester_quorum_tool_defaults_version_and_encodes_expected_prefix() {
        let ctx = context_with_policy(Policy::default());
        let result = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({ "signatures": [format!("0x{}", "11".repeat(65))] }),
        )
        .expect("default version should encode attester proof");

        assert_eq!(result["version"], json!(1));
        assert_eq!(result["signature_count"], json!(1));
        let proof_hex = result["proof_hex"]
            .as_str()
            .expect("proof_hex should be string");
        assert!(
            proof_hex.starts_with("0x0104"),
            "unexpected proof prefix: {proof_hex}"
        );
        // 1 byte version + 1 byte compact-len + 65-byte signature = 67 bytes -> 134 hex chars + 0x.
        assert_eq!(proof_hex.len(), 136, "unexpected proof hex length");
    }

    #[test]
    fn attester_quorum_tool_encodes_empty_signature_set() {
        let ctx = context_with_policy(Policy::default());
        let result = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 1,
                "signatures": []
            }),
        )
        .expect("empty signature set should still encode");

        assert_eq!(result["version"], json!(1));
        assert_eq!(result["signature_count"], json!(0));
        assert_eq!(result["proof_hex"], json!("0x0100")); // version=1, compact vec len=0
    }

    #[test]
    fn attester_quorum_tool_encodes_compact_len_mode1_boundary() {
        let ctx = context_with_policy(Policy::default());
        let signature = format!("0x{}", "11".repeat(65));
        let signatures = std::iter::repeat(signature).take(64).collect::<Vec<_>>();
        let result = dispatch(
            &ctx,
            "sccp_encode_attester_quorum_proof",
            &json!({
                "version": 1,
                "signatures": signatures
            }),
        )
        .expect("64 signatures should encode using compact mode=1 length");

        assert_eq!(result["version"], json!(1));
        assert_eq!(result["signature_count"], json!(64));
        let proof_hex = result["proof_hex"]
            .as_str()
            .expect("proof_hex should be string");
        // 0x01(version), 0x01 0x01(compact-u32 len=64), then 64x65-byte signatures.
        assert!(
            proof_hex.starts_with("0x010101"),
            "unexpected proof prefix: {proof_hex}"
        );
        let expected_len = 2 + ((1 + 2 + (64 * 65)) * 2);
        assert_eq!(proof_hex.len(), expected_len, "unexpected proof hex length");
    }

    #[test]
    fn sora_build_call_requires_pallet_index_from_args_or_profile() {
        let ctx = context_with_sora_network(None);
        let error = dispatch(
            &ctx,
            "sora_sccp_build_call",
            &json!({
                "network": "sora_local",
                "call_name": "add_token",
                "args": {
                    "asset_id": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                }
            }),
        )
        .expect_err("missing pallet index must fail");

        assert!(
            error.to_string().contains("missing sccp_pallet_index"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn sora_build_call_uses_explicit_pallet_index_override() {
        let ctx = context_with_sora_network(None);
        let result = dispatch(
            &ctx,
            "sora_sccp_build_call",
            &json!({
                "network": "sora_local",
                "call_name": "add_token",
                "pallet_index": 77,
                "args": {
                    "asset_id": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                }
            }),
        )
        .expect("explicit pallet index should encode call");

        assert_eq!(result["pallet_index"], json!(77));
        assert_eq!(result["call_index"], json!(0));
        assert_eq!(result["call_data_len"], json!(34));
        let call_data = result["call_data_hex"]
            .as_str()
            .expect("call_data_hex must be present");
        assert!(
            call_data.starts_with("0x4d00"),
            "unexpected calldata: {call_data}"
        );
    }
}
