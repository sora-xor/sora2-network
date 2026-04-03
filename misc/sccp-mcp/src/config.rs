use crate::error::{AppError, AppResult};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::net::IpAddr;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub policy: Policy,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub deployment: DeploymentPolicy,
    #[serde(default)]
    pub networks: BTreeMap<String, NetworkProfile>,
}

pub const DEFAULT_READ_ONLY_ALLOW_TOOLS: &[&str] = &[
    "sccp_list_networks",
    "sccp_health",
    "sccp_get_message_id",
    "sccp_validate_payload",
    "sccp_list_supported_calls",
    "sccp_get_token_state",
    "sccp_get_message_status",
    "nexus_sccp_get_bundle",
    "nexus_sccp_build_sora_call",
    "sora_sccp_build_call",
    "sora_sccp_estimate_fee",
    "evm_sccp_read_contract",
    "evm_sccp_build_burn_proof",
    "evm_sccp_build_tx",
    "sol_sccp_get_account",
    "sol_sccp_build_transaction",
    "ton_sccp_get_method",
    "ton_sccp_build_message",
];

pub const MUTATING_TOOL_NAMES: &[&str] = &[
    "sora_sccp_submit_signed_extrinsic",
    "evm_sccp_submit_signed_tx",
    "sol_sccp_submit_signed_transaction",
    "ton_sccp_submit_signed_message",
];

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
    Nexus,
    Evm,
    Solana,
    Ton,
}

impl std::fmt::Display for NetworkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            NetworkKind::Sora => "sora",
            NetworkKind::Nexus => "nexus",
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

#[derive(Debug, Clone, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub allow_tools: Vec<String>,
    #[serde(default)]
    pub deny_tools: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Auth {
    #[serde(default)]
    pub required_token: Option<String>,
    #[serde(default = "default_required_token_env")]
    pub required_token_env: String,
    #[serde(default = "default_min_required_token_bytes")]
    pub min_required_token_bytes: usize,
    #[serde(default = "default_max_token_bytes")]
    pub max_token_bytes: usize,
}

fn default_required_token_env() -> String {
    "SCCP_MCP_AUTH_TOKEN".to_owned()
}

fn default_min_required_token_bytes() -> usize {
    32
}

fn default_max_token_bytes() -> usize {
    512
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            required_token: None,
            required_token_env: default_required_token_env(),
            min_required_token_bytes: default_min_required_token_bytes(),
            max_token_bytes: default_max_token_bytes(),
        }
    }
}

impl Auth {
    fn validate_token_strength(&self, token: &str) -> AppResult<()> {
        if self.min_required_token_bytes < 16 {
            return Err(AppError::Config(
                "[auth].min_required_token_bytes must be at least 16".to_owned(),
            ));
        }
        if self.max_token_bytes < self.min_required_token_bytes {
            return Err(AppError::Config(format!(
                "[auth].max_token_bytes ({}) must be >= [auth].min_required_token_bytes ({})",
                self.max_token_bytes, self.min_required_token_bytes
            )));
        }
        let token_len = token.as_bytes().len();
        if token_len > self.max_token_bytes {
            return Err(AppError::Config(format!(
                "resolved auth token too long: {token_len} bytes; max allowed {} bytes",
                self.max_token_bytes
            )));
        }
        if token_len < self.min_required_token_bytes {
            return Err(AppError::Config(format!(
                "resolved auth token too short: {token_len} bytes; require at least {} bytes",
                self.min_required_token_bytes
            )));
        }
        Ok(())
    }

    fn inline_token(&self) -> Option<String> {
        self.required_token
            .as_ref()
            .map(|token| token.trim().to_owned())
            .filter(|token| !token.is_empty())
    }

    pub fn resolve_required_token(&self) -> AppResult<String> {
        if let Some(token) = self.inline_token() {
            self.validate_token_strength(&token)?;
            return Ok(token);
        }

        let env_key = self.required_token_env.trim();
        if env_key.is_empty() {
            return Err(AppError::Config(
                "[auth].required_token_env must be non-empty when [auth].required_token is unset"
                    .to_owned(),
            ));
        }

        match std::env::var(env_key) {
            Ok(value) => {
                let token = value.trim();
                if token.is_empty() {
                    Err(AppError::Config(format!(
                        "auth token environment variable {env_key} is set but empty"
                    )))
                } else {
                    self.validate_token_strength(token)?;
                    Ok(token.to_owned())
                }
            }
            Err(_) => Err(AppError::Config(format!(
                "missing required auth token: set [auth].required_token or environment variable {env_key}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentPolicy {
    #[serde(default)]
    pub allow_non_private_rpc: bool,
    #[serde(default)]
    pub allow_mutating_tools: bool,
}

impl Default for DeploymentPolicy {
    fn default() -> Self {
        Self {
            allow_non_private_rpc: false,
            allow_mutating_tools: false,
        }
    }
}

impl Policy {
    pub fn allows(&self, tool_name: &str) -> bool {
        if self.deny_tools.iter().any(|name| name == tool_name) {
            return false;
        }
        if self.allow_tools.is_empty() {
            return false;
        }
        self.allow_tools.iter().any(|name| name == tool_name)
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            allow_tools: DEFAULT_READ_ONLY_ALLOW_TOOLS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            deny_tools: Vec::new(),
        }
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

    pub fn enabled_mutating_tools(&self) -> Vec<&'static str> {
        MUTATING_TOOL_NAMES
            .iter()
            .copied()
            .filter(|tool_name| self.policy.allows(tool_name))
            .collect()
    }

    pub fn network_names_with_non_private_rpc(&self) -> Vec<String> {
        self.networks
            .iter()
            .filter_map(|(name, profile)| {
                if is_private_or_loopback_rpc_url(&profile.rpc_url) {
                    None
                } else {
                    Some(name.clone())
                }
            })
            .collect()
    }

    pub fn resolve_auth_token_for_startup(&mut self) -> AppResult<()> {
        let resolved = self.auth.resolve_required_token()?;
        self.auth.required_token = Some(resolved);
        Ok(())
    }

    pub fn validate_startup_policy(&self) -> AppResult<()> {
        let mut violations = Vec::new();

        let mutating_tools = self.enabled_mutating_tools();
        let non_private_rpc = self.network_names_with_non_private_rpc();
        match self.auth.inline_token() {
            Some(token) => {
                if let Err(err) = self.auth.validate_token_strength(&token) {
                    violations.push(err.to_string());
                }
            }
            None => {
                violations.push("missing required MCP auth token; set [auth].required_token or [auth].required_token_env".to_owned());
            }
        }

        if !mutating_tools.is_empty() && !self.deployment.allow_mutating_tools {
            violations.push(format!(
                "submit-capable tools are enabled ({}) but [deployment].allow_mutating_tools is false",
                mutating_tools.join(", ")
            ));
        }

        if !non_private_rpc.is_empty() && !self.deployment.allow_non_private_rpc {
            violations.push(format!(
                "non-private RPC URLs configured for networks ({}) but [deployment].allow_non_private_rpc is false",
                non_private_rpc.join(", ")
            ));
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(AppError::Config(format!(
                "unsafe deployment config blocked: {}",
                violations.join("; ")
            )))
        }
    }
}

fn is_private_or_loopback_rpc_url(url: &str) -> bool {
    let host = match extract_host(url) {
        Some(host) => host,
        None => return false,
    };
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private(),
        Ok(IpAddr::V6(ip)) => {
            let first_segment = ip.segments()[0];
            let is_unique_local = (first_segment & 0xfe00) == 0xfc00;
            ip.is_loopback() || is_unique_local
        }
        Err(_) => false,
    }
}

fn extract_host(url: &str) -> Option<&str> {
    let (_, after_scheme) = url.split_once("://")?;
    let authority = after_scheme.split('/').next()?;
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    if authority.starts_with('[') {
        let closing = authority.find(']')?;
        return authority.get(1..closing);
    }
    authority.split(':').next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const STRONG_TEST_TOKEN: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn unique_temp_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("sccp_mcp_{name}_{nanos}.toml"))
    }

    fn unique_env_key(name: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        format!("SCCP_MCP_TEST_{name}_{nanos}")
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
    fn policy_deny_by_default_when_allow_list_is_empty() {
        let policy = Policy {
            allow_tools: vec![],
            deny_tools: vec!["tool.blocked".to_owned()],
        };
        assert!(!policy.allows("tool.any"));
        assert!(!policy.allows("tool.blocked"));
    }

    #[test]
    fn policy_default_is_read_only_allow_list() {
        let policy = Policy::default();
        assert!(policy.allows("sccp_list_networks"));
        assert!(policy.allows("evm_sccp_read_contract"));
        assert!(policy.allows("evm_sccp_build_burn_proof"));
        assert!(!policy.allows("sora_sccp_submit_signed_extrinsic"));
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
    fn load_config_rejects_unknown_network_kind() {
        let path = unique_temp_path("bad_kind");
        let toml_text = r#"
            [networks.invalid]
            kind = "not_a_kind"
            rpc_url = "http://127.0.0.1:9933"
        "#;
        fs::write(&path, toml_text).expect("temp config should be writable");

        let err = Config::load(&path).expect_err("unknown network kind must fail to parse");
        assert!(
            err.to_string().contains("failed to parse"),
            "unexpected error: {err}"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_config_rejects_network_missing_kind() {
        let path = unique_temp_path("missing_kind");
        let toml_text = r#"
            [networks.invalid]
            rpc_url = "http://127.0.0.1:9933"
        "#;
        fs::write(&path, toml_text).expect("temp config should be writable");

        let err = Config::load(&path).expect_err("network kind is required");
        assert!(
            err.to_string().contains("failed to parse"),
            "unexpected error: {err}"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_config_rejects_network_missing_rpc_url() {
        let path = unique_temp_path("missing_rpc");
        let toml_text = r#"
            [networks.invalid]
            kind = "sora"
        "#;
        fs::write(&path, toml_text).expect("temp config should be writable");

        let err = Config::load(&path).expect_err("network rpc_url is required");
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

            [deployment]
            allow_non_private_rpc = true
            allow_mutating_tools = false

            [auth]
            required_token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

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
        assert_eq!(cfg.auth.required_token.as_deref(), Some(STRONG_TEST_TOKEN));
        assert!(cfg.deployment.allow_non_private_rpc);
        assert!(!cfg.deployment.allow_mutating_tools);
        assert!(cfg.policy.allows("tool.allowed"));
        assert!(!cfg.policy.allows("tool.denied"));
        assert!(!cfg.policy.allows("tool.other"));

        let net = cfg.network("sora_main").expect("network should exist");
        assert_eq!(net.sccp_pallet_index, Some(77));
        assert_eq!(net.block_number_bytes, 8);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn resolve_auth_token_for_startup_prefers_inline_token() {
        let mut cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: Some(STRONG_TEST_TOKEN.to_owned()),
                required_token_env: "IGNORED_ENV_KEY".to_owned(),
                min_required_token_bytes: default_min_required_token_bytes(),
                max_token_bytes: default_max_token_bytes(),
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        cfg.resolve_auth_token_for_startup()
            .expect("inline token should resolve");
        assert_eq!(cfg.auth.required_token.as_deref(), Some(STRONG_TEST_TOKEN));
    }

    #[test]
    fn resolve_auth_token_for_startup_supports_env_token() {
        let env_key = unique_env_key("AUTH_TOKEN");
        std::env::set_var(&env_key, STRONG_TEST_TOKEN);

        let mut cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: None,
                required_token_env: env_key.clone(),
                min_required_token_bytes: default_min_required_token_bytes(),
                max_token_bytes: default_max_token_bytes(),
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        cfg.resolve_auth_token_for_startup()
            .expect("env token should resolve");
        assert_eq!(cfg.auth.required_token.as_deref(), Some(STRONG_TEST_TOKEN));

        std::env::remove_var(env_key);
    }

    #[test]
    fn resolve_auth_token_for_startup_fails_when_missing() {
        let env_key = unique_env_key("MISSING_AUTH_TOKEN");
        std::env::remove_var(&env_key);

        let mut cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: None,
                required_token_env: env_key.clone(),
                min_required_token_bytes: default_min_required_token_bytes(),
                max_token_bytes: default_max_token_bytes(),
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        let err = cfg
            .resolve_auth_token_for_startup()
            .expect_err("missing auth token should fail closed");
        assert!(
            err.to_string().contains("missing required auth token"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_auth_token_for_startup_fails_when_token_is_too_short() {
        let mut cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: Some("short-token".to_owned()),
                required_token_env: default_required_token_env(),
                min_required_token_bytes: default_min_required_token_bytes(),
                max_token_bytes: default_max_token_bytes(),
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        let err = cfg
            .resolve_auth_token_for_startup()
            .expect_err("short auth token must fail closed");
        assert!(
            err.to_string().contains("resolved auth token too short"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn default_limits_values_are_stable() {
        let defaults = Limits::default();
        assert_eq!(defaults.max_call_bytes, 131_072);
        assert_eq!(defaults.max_proof_bytes, 1_048_576);
        assert_eq!(defaults.max_request_bytes, 4_194_304);
    }

    #[test]
    fn default_auth_limits_are_stable() {
        let defaults = Auth::default();
        assert_eq!(defaults.min_required_token_bytes, 32);
        assert_eq!(defaults.max_token_bytes, 512);
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
            auth: Auth::default(),
            deployment: DeploymentPolicy::default(),
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
            auth: Auth::default(),
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };
        assert!(cfg.list_network_names().is_empty());
    }

    #[test]
    fn enabled_mutating_tools_reports_only_explicitly_allowed_submit_tools() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth::default(),
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };
        assert!(cfg.enabled_mutating_tools().is_empty());

        let cfg = Config {
            limits: Limits::default(),
            policy: Policy {
                allow_tools: vec![
                    "sccp_list_networks".to_owned(),
                    "sora_sccp_submit_signed_extrinsic".to_owned(),
                ],
                deny_tools: vec![],
            },
            auth: Auth::default(),
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };
        assert_eq!(
            cfg.enabled_mutating_tools(),
            vec!["sora_sccp_submit_signed_extrinsic"]
        );
    }

    #[test]
    fn network_names_with_non_private_rpc_finds_public_endpoints() {
        let mut networks = BTreeMap::new();
        networks.insert(
            "local".to_owned(),
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
        networks.insert(
            "public".to_owned(),
            NetworkProfile {
                kind: NetworkKind::Evm,
                rpc_url: "https://rpc.sepolia.org".to_owned(),
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
            "private".to_owned(),
            NetworkProfile {
                kind: NetworkKind::Evm,
                rpc_url: "http://10.0.0.5:8545".to_owned(),
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
            auth: Auth::default(),
            deployment: DeploymentPolicy::default(),
            networks,
        };
        assert_eq!(
            cfg.network_names_with_non_private_rpc(),
            vec!["public".to_owned()]
        );
    }

    #[test]
    fn network_kind_display_strings_are_stable() {
        assert_eq!(NetworkKind::Sora.to_string(), "sora");
        assert_eq!(NetworkKind::Evm.to_string(), "evm");
        assert_eq!(NetworkKind::Solana.to_string(), "solana");
        assert_eq!(NetworkKind::Ton.to_string(), "ton");
    }

    #[test]
    fn startup_policy_blocks_mutating_tools_without_explicit_ack() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy {
                allow_tools: vec!["sora_sccp_submit_signed_extrinsic".to_owned()],
                deny_tools: vec![],
            },
            auth: Auth::default(),
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("mutating tools should be blocked without explicit deployment ack");
        let message = err.to_string();
        assert!(
            message.contains("allow_mutating_tools"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn startup_policy_requires_auth_token_for_baseline_profile() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth::default(),
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("baseline profile must fail closed without auth token");
        assert!(
            err.to_string().contains("missing required MCP auth token"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn startup_policy_allows_read_only_profile_with_auth_token() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: Some(STRONG_TEST_TOKEN.to_owned()),
                ..Auth::default()
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        cfg.validate_startup_policy()
            .expect("read-only baseline should be allowed with auth token");
    }

    #[test]
    fn startup_policy_allows_mutating_tools_with_explicit_ack() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy {
                allow_tools: vec!["sora_sccp_submit_signed_extrinsic".to_owned()],
                deny_tools: vec![],
            },
            auth: Auth {
                required_token: Some(STRONG_TEST_TOKEN.to_owned()),
                ..Auth::default()
            },
            deployment: DeploymentPolicy {
                allow_non_private_rpc: false,
                allow_mutating_tools: true,
            },
            networks: BTreeMap::new(),
        };

        cfg.validate_startup_policy()
            .expect("mutating tools should be allowed with explicit deployment ack");
    }

    #[test]
    fn startup_policy_blocks_public_rpc_without_explicit_ack() {
        let mut networks = BTreeMap::new();
        networks.insert(
            "public".to_owned(),
            NetworkProfile {
                kind: NetworkKind::Evm,
                rpc_url: "https://rpc.sepolia.org".to_owned(),
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
            auth: Auth::default(),
            deployment: DeploymentPolicy::default(),
            networks,
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("public RPC should be blocked without explicit deployment ack");
        let message = err.to_string();
        assert!(
            message.contains("allow_non_private_rpc"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn startup_policy_allows_public_rpc_with_explicit_ack() {
        let mut networks = BTreeMap::new();
        networks.insert(
            "public".to_owned(),
            NetworkProfile {
                kind: NetworkKind::Evm,
                rpc_url: "https://rpc.sepolia.org".to_owned(),
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
            auth: Auth {
                required_token: Some(STRONG_TEST_TOKEN.to_owned()),
                ..Auth::default()
            },
            deployment: DeploymentPolicy {
                allow_non_private_rpc: true,
                allow_mutating_tools: false,
            },
            networks,
        };

        cfg.validate_startup_policy()
            .expect("public RPC should be allowed with explicit deployment ack");
    }

    #[test]
    fn startup_policy_requires_auth_token_for_mutating_override() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy {
                allow_tools: vec!["sora_sccp_submit_signed_extrinsic".to_owned()],
                deny_tools: vec![],
            },
            auth: Auth::default(),
            deployment: DeploymentPolicy {
                allow_non_private_rpc: false,
                allow_mutating_tools: true,
            },
            networks: BTreeMap::new(),
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("mutating override without [auth].required_token should fail closed");
        assert!(
            err.to_string().contains("[auth].required_token"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn startup_policy_requires_auth_token_for_public_rpc_override() {
        let mut networks = BTreeMap::new();
        networks.insert(
            "public".to_owned(),
            NetworkProfile {
                kind: NetworkKind::Evm,
                rpc_url: "https://rpc.sepolia.org".to_owned(),
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
            auth: Auth::default(),
            deployment: DeploymentPolicy {
                allow_non_private_rpc: true,
                allow_mutating_tools: false,
            },
            networks,
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("public RPC override without [auth].required_token should fail closed");
        assert!(
            err.to_string().contains("[auth].required_token"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn startup_policy_rejects_short_auth_token() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: Some("short-token".to_owned()),
                ..Auth::default()
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("short auth token should fail startup policy");
        assert!(
            err.to_string().contains("resolved auth token too short"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn startup_policy_rejects_overlong_auth_token() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: Some("a".repeat(513)),
                ..Auth::default()
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("overlong auth token should fail startup policy");
        assert!(
            err.to_string().contains("resolved auth token too long"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn startup_policy_rejects_invalid_min_required_token_bytes() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: Some(STRONG_TEST_TOKEN.to_owned()),
                min_required_token_bytes: 8,
                ..Auth::default()
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("too-small min_required_token_bytes must fail closed");
        assert!(
            err.to_string().contains("min_required_token_bytes"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn startup_policy_rejects_max_token_bytes_below_minimum() {
        let cfg = Config {
            limits: Limits::default(),
            policy: Policy::default(),
            auth: Auth {
                required_token: Some(STRONG_TEST_TOKEN.to_owned()),
                min_required_token_bytes: 64,
                max_token_bytes: 32,
                ..Auth::default()
            },
            deployment: DeploymentPolicy::default(),
            networks: BTreeMap::new(),
        };

        let err = cfg
            .validate_startup_policy()
            .expect_err("max_token_bytes below min_required_token_bytes must fail closed");
        assert!(
            err.to_string().contains("max_token_bytes"),
            "unexpected error: {err}"
        );
    }
}
