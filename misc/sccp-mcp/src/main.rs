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
    let config = Config::load(&config_path)
        .map_err(|err| anyhow::anyhow!("failed to load config {}: {err}", config_path.display()))?;

    eprintln!(
        "sccp-mcp starting with {} network profile(s) from {}",
        config.networks.len(),
        config_path.display()
    );

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
