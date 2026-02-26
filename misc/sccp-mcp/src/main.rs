mod config;
mod error;
mod mcp;
mod payload;
mod rpc_client;
mod sora_calls;
mod substrate_storage;
mod tools;

use crate::config::Config;
use crate::tools::ToolContext;
use std::path::{Path, PathBuf};

fn main() -> anyhow::Result<()> {
    let config_path = resolve_config_path();
    let mut config = Config::load(&config_path)
        .map_err(|err| anyhow::anyhow!("failed to load config {}: {err}", config_path.display()))?;
    config
        .resolve_auth_token_for_startup()
        .map_err(|err| anyhow::anyhow!("failed to resolve auth token for startup: {err}"))?;
    config
        .validate_startup_policy()
        .map_err(|err| anyhow::anyhow!("refusing to start with unsafe config: {err}"))?;

    eprintln!(
        "sccp-mcp starting with {} network profile(s) from {}",
        config.networks.len(),
        config_path.display()
    );
    let mutating_tools = config.enabled_mutating_tools();
    if !mutating_tools.is_empty() {
        eprintln!(
            "WARNING: submit-capable tools enabled with explicit deployment override: {}",
            mutating_tools.join(", ")
        );
        eprintln!(
            "WARNING: keep MCP stdio private and enforce authenticated gateway/service isolation."
        );
    }

    let public_rpc_networks = config.network_names_with_non_private_rpc();
    if !public_rpc_networks.is_empty() {
        eprintln!(
            "WARNING: non-private RPC URLs configured with explicit deployment override for networks: {}",
            public_rpc_networks.join(", ")
        );
        eprintln!(
            "WARNING: confirm gateway authz, firewall policy, and credential scoping before production use."
        );
    }

    let ctx = ToolContext { config };
    mcp::run_server(ctx)
}

fn resolve_config_path() -> PathBuf {
    if let Ok(path) = std::env::var("SCCP_MCP_CONFIG") {
        return PathBuf::from(path);
    }

    if Path::new("config.toml").exists() {
        return PathBuf::from("config.toml");
    }

    PathBuf::from("config.example.toml")
}
