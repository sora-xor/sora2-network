use crate::error::{AppError, AppResult};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub policy: Policy,
    #[serde(default)]
    pub networks: BTreeMap<String, NetworkProfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkProfile {
    pub kind: NetworkKind,
    pub rpc_url: String,
    #[serde(default)]
    pub ws_url: Option<String>,
    #[serde(default)]
    pub chain_id: Option<u64>,
    #[serde(default)]
    pub genesis_hash: Option<String>,
    #[serde(default)]
    pub ss58_prefix: Option<u16>,
    #[serde(default)]
    pub sccp_pallet_index: Option<u8>,
    #[serde(default = "default_block_number_bytes")]
    pub block_number_bytes: u8,
    #[serde(default)]
    pub router_address: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

fn default_block_number_bytes() -> u8 {
    4
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkKind {
    Sora,
    Evm,
    Solana,
    Ton,
}

impl std::fmt::Display for NetworkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            NetworkKind::Sora => "sora",
            NetworkKind::Evm => "evm",
            NetworkKind::Solana => "solana",
            NetworkKind::Ton => "ton",
        };
        write!(f, "{text}")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Limits {
    #[serde(default = "default_max_call_bytes")]
    pub max_call_bytes: usize,
    #[serde(default = "default_max_proof_bytes")]
    pub max_proof_bytes: usize,
    #[serde(default = "default_max_request_bytes")]
    pub max_request_bytes: usize,
}

fn default_max_call_bytes() -> usize {
    131_072
}

fn default_max_proof_bytes() -> usize {
    1_048_576
}

fn default_max_request_bytes() -> usize {
    4_194_304
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_call_bytes: default_max_call_bytes(),
            max_proof_bytes: default_max_proof_bytes(),
            max_request_bytes: default_max_request_bytes(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub allow_tools: Vec<String>,
    #[serde(default)]
    pub deny_tools: Vec<String>,
}

impl Policy {
    pub fn allows(&self, tool_name: &str) -> bool {
        if self.deny_tools.iter().any(|name| name == tool_name) {
            return false;
        }
        if self.allow_tools.is_empty() {
            return true;
        }
        self.allow_tools.iter().any(|name| name == tool_name)
    }
}

impl Config {
    pub fn load(path: &Path) -> AppResult<Self> {
        let content = fs::read_to_string(path).map_err(|err| {
            AppError::Config(format!("failed to read {}: {err}", path.to_string_lossy()))
        })?;
        let cfg: Config = toml::from_str(&content).map_err(|err| {
            AppError::Config(format!(
                "failed to parse {} as TOML: {err}",
                path.to_string_lossy()
            ))
        })?;
        Ok(cfg)
    }

    pub fn network(&self, name: &str) -> AppResult<&NetworkProfile> {
        self.networks
            .get(name)
            .ok_or_else(|| AppError::UnknownNetwork(name.to_owned()))
    }

    pub fn list_network_names(&self) -> Vec<String> {
        self.networks.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("sccp_mcp_{name}_{nanos}.toml"))
    }

    #[test]
    fn policy_deny_overrides_allow() {
        let policy = Policy {
            allow_tools: vec!["tool.a".to_owned()],
            deny_tools: vec!["tool.a".to_owned()],
        };
        assert!(!policy.allows("tool.a"));
    }

    #[test]
    fn policy_allow_list_restricts_when_non_empty() {
        let policy = Policy {
            allow_tools: vec!["tool.a".to_owned()],
            deny_tools: vec![],
        };
        assert!(policy.allows("tool.a"));
        assert!(!policy.allows("tool.b"));
    }

    #[test]
    fn policy_allows_when_allow_list_is_empty_and_not_denied() {
        let policy = Policy {
            allow_tools: vec![],
            deny_tools: vec!["tool.blocked".to_owned()],
        };
        assert!(policy.allows("tool.any"));
        assert!(!policy.allows("tool.blocked"));
    }

    #[test]
    fn load_config_and_lookup_network() {
        let path = unique_temp_path("load_ok");
        let toml_text = r#"
            [networks.sora_testnet]
            kind = "sora"
            rpc_url = "http://127.0.0.1:9933"
        "#;
        fs::write(&path, toml_text).expect("temp config should be writable");

        let cfg = Config::load(&path).expect("config should load");
        let net = cfg.network("sora_testnet").expect("network should exist");
        assert_eq!(net.kind, NetworkKind::Sora);
        assert_eq!(net.block_number_bytes, 4);
        assert!(cfg.network("missing").is_err());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_config_reports_parse_errors() {
        let path = unique_temp_path("parse_err");
        fs::write(&path, "this is not toml").expect("temp config should be writable");
        let err = Config::load(&path).expect_err("invalid TOML must fail");
        assert!(
            err.to_string().contains("failed to parse"),
            "unexpected error: {err}"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_config_reports_read_errors() {
        let path = unique_temp_path("missing");
        let err = Config::load(&path).expect_err("missing file must fail");
        assert!(
            err.to_string().contains("failed to read"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn load_config_parses_limits_policy_and_network_overrides() {
        let path = unique_temp_path("limits_policy");
        let toml_text = r#"
            [limits]
            max_call_bytes = 2048
            max_proof_bytes = 4096
            max_request_bytes = 8192

            [policy]
            allow_tools = ["tool.allowed"]
            deny_tools = ["tool.denied"]

            [networks.sora_main]
            kind = "sora"
            rpc_url = "http://127.0.0.1:9933"
            sccp_pallet_index = 77
            block_number_bytes = 8
        "#;
        fs::write(&path, toml_text).expect("temp config should be writable");

        let cfg = Config::load(&path).expect("config should load");
        assert_eq!(cfg.limits.max_call_bytes, 2048);
        assert_eq!(cfg.limits.max_proof_bytes, 4096);
        assert_eq!(cfg.limits.max_request_bytes, 8192);
        assert!(cfg.policy.allows("tool.allowed"));
        assert!(!cfg.policy.allows("tool.denied"));
        assert!(!cfg.policy.allows("tool.other"));

        let net = cfg.network("sora_main").expect("network should exist");
        assert_eq!(net.sccp_pallet_index, Some(77));
        assert_eq!(net.block_number_bytes, 8);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn default_limits_values_are_stable() {
        let defaults = Limits::default();
        assert_eq!(defaults.max_call_bytes, 131_072);
        assert_eq!(defaults.max_proof_bytes, 1_048_576);
        assert_eq!(defaults.max_request_bytes, 4_194_304);
    }

    #[test]
    fn list_network_names_is_sorted_by_key() {
        let mut networks = BTreeMap::new();
        networks.insert(
            "zeta".to_owned(),
            NetworkProfile {
                kind: NetworkKind::Evm,
                rpc_url: "http://127.0.0.1:8545".to_owned(),
                ws_url: None,
                chain_id: None,
                genesis_hash: None,
                ss58_prefix: None,
                sccp_pallet_index: None,
                block_number_bytes: 4,
                router_address: None,
                notes: None,
            },
        );
        networks.insert(
            "alpha".to_owned(),
            NetworkProfile {
                kind: NetworkKind::Sora,
                rpc_url: "http://127.0.0.1:9933".to_owned(),
                ws_url: None,
                chain_id: None,
                genesis_hash: None,
                ss58_prefix: None,
                sccp_pallet_index: None,
                block_number_bytes: 4,
                router_address: None,
                notes: None,
            },
        );

        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            networks,
        };
        assert_eq!(
            cfg.list_network_names(),
            vec!["alpha".to_owned(), "zeta".to_owned()]
        );
    }

    #[test]
    fn list_network_names_empty_for_empty_config() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            networks: BTreeMap::new(),
        };
        assert!(cfg.list_network_names().is_empty());
    }

    #[test]
    fn network_kind_display_strings_are_stable() {
        assert_eq!(NetworkKind::Sora.to_string(), "sora");
        assert_eq!(NetworkKind::Evm.to_string(), "evm");
        assert_eq!(NetworkKind::Solana.to_string(), "solana");
        assert_eq!(NetworkKind::Ton.to_string(), "ton");
    }
}
