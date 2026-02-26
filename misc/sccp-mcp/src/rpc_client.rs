use crate::error::{AppError, AppResult};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static CIRCUIT_STATES: Lazy<Mutex<HashMap<String, CircuitState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static RPC_INFLIGHT: AtomicUsize = AtomicUsize::new(0);
static RPC_INFLIGHT_BY_ENDPOINT: Lazy<Mutex<HashMap<String, usize>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static RPC_INFLIGHT_BY_PRINCIPAL: Lazy<Mutex<HashMap<String, usize>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static RPC_INFLIGHT_BY_SCOPE: Lazy<Mutex<HashMap<String, usize>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static RPC_INFLIGHT_BY_METHOD: Lazy<Mutex<HashMap<String, usize>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static RPC_ADMISSION_QUEUE: Lazy<AdmissionQueue> = Lazy::new(AdmissionQueue::default);
thread_local! {
    static RPC_FAIRNESS_PRINCIPAL: RefCell<Option<String>> = const { RefCell::new(None) };
    static RPC_FAIRNESS_SCOPE: RefCell<Option<String>> = const { RefCell::new(None) };
}

const DEFAULT_RPC_CONNECT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_RPC_IO_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_RPC_MAX_RETRIES: u32 = 1;
const DEFAULT_RPC_RETRY_BACKOFF_MS: u64 = 250;
const DEFAULT_RPC_CIRCUIT_BREAKER_THRESHOLD: u32 = 5;
const DEFAULT_RPC_CIRCUIT_BREAKER_COOLDOWN_MS: u64 = 30_000;
const DEFAULT_RPC_MAX_INFLIGHT: usize = 32;
const DEFAULT_RPC_MAX_INFLIGHT_PER_ENDPOINT: usize = 16;
const DEFAULT_RPC_MAX_INFLIGHT_PER_PRINCIPAL: usize = 12;
const DEFAULT_RPC_MAX_INFLIGHT_PER_SCOPE: usize = 12;
const DEFAULT_RPC_MAX_INFLIGHT_PER_METHOD: usize = 8;
const MAX_RPC_MAX_INFLIGHT: usize = 1024;
const DEFAULT_RPC_QUEUE_ENABLE: bool = false;
const DEFAULT_RPC_QUEUE_MAX_PENDING: usize = 256;
const DEFAULT_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL: usize = 32;
const DEFAULT_RPC_QUEUE_WAIT_TIMEOUT_MS: u64 = 200;
const DEFAULT_RPC_QUEUE_DRR_QUANTUM: usize = 1;
const DEFAULT_RPC_PRINCIPAL_WEIGHT_DEFAULT: usize = 1;
const MAX_RPC_QUEUE_PENDING: usize = 4096;
const MAX_RPC_QUEUE_DRR_QUANTUM: usize = 1024;
const MAX_RPC_PRINCIPAL_WEIGHT: usize = 1024;

type RequestId = u64;

#[derive(Debug, Clone, Copy)]
struct RetryPolicy {
    max_retries: u32,
    backoff: Duration,
}

#[derive(Debug, Clone, Copy)]
struct CircuitBreakerPolicy {
    failure_threshold: u32,
    cooldown: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
struct CircuitState {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

#[derive(Debug, Clone)]
struct RpcQueuePolicy {
    enabled: bool,
    max_pending: usize,
    max_pending_per_principal: usize,
    wait_timeout: Duration,
    drr_quantum: usize,
    principal_weight_default: usize,
    principal_weights: HashMap<String, usize>,
}

impl RpcQueuePolicy {
    fn weight_for(&self, principal: &str) -> usize {
        self.principal_weights
            .get(principal)
            .copied()
            .unwrap_or(self.principal_weight_default)
    }

    fn deficit_increment_for(&self, principal: &str) -> usize {
        self.drr_quantum.saturating_mul(self.weight_for(principal))
    }
}

#[derive(Debug, Default)]
struct AdmissionState {
    pending_by_principal: HashMap<String, VecDeque<RequestId>>,
    principal_ring: VecDeque<String>,
    deficit_by_principal: HashMap<String, usize>,
    pending_count_by_principal: HashMap<String, usize>,
    pending_total: usize,
    admitted_total: usize,
    admitted_requests: HashSet<RequestId>,
    next_request_id: RequestId,
}

impl AdmissionState {
    fn pending_for_principal(&self, principal: &str) -> usize {
        *self.pending_count_by_principal.get(principal).unwrap_or(&0)
    }

    fn has_global_capacity(&self, global_limit: usize) -> bool {
        global_limit > 0 && self.admitted_total < global_limit
    }

    fn allocate_request_id(&mut self) -> RequestId {
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.next_request_id
    }

    fn enqueue_request(&mut self, principal: &str, request_id: RequestId) {
        let queue = self
            .pending_by_principal
            .entry(principal.to_owned())
            .or_default();
        let was_empty = queue.is_empty();
        queue.push_back(request_id);
        if was_empty {
            self.principal_ring.push_back(principal.to_owned());
        }
        *self
            .pending_count_by_principal
            .entry(principal.to_owned())
            .or_insert(0) += 1;
        self.pending_total = self.pending_total.saturating_add(1);
    }

    fn clear_principal_if_idle(&mut self, principal: &str) {
        if self.pending_for_principal(principal) == 0 {
            self.principal_ring.retain(|entry| entry != principal);
            self.pending_by_principal.remove(principal);
            self.pending_count_by_principal.remove(principal);
            self.deficit_by_principal.remove(principal);
        }
    }

    fn next_admissible_principal(&mut self, policy: &RpcQueuePolicy) -> Option<String> {
        let ring_len = self.principal_ring.len();
        for _ in 0..ring_len {
            let principal = self.principal_ring.pop_front()?;
            let pending = self.pending_for_principal(&principal);
            if pending == 0 {
                self.clear_principal_if_idle(&principal);
                continue;
            }

            let deficit = self
                .deficit_by_principal
                .entry(principal.clone())
                .or_insert(0);
            if *deficit == 0 {
                *deficit = policy.deficit_increment_for(&principal);
            }
            if *deficit >= 1 {
                *deficit -= 1;
                if pending > 1 && *deficit >= 1 {
                    self.principal_ring.push_front(principal.clone());
                } else {
                    self.principal_ring.push_back(principal.clone());
                }
                return Some(principal);
            }
            self.principal_ring.push_back(principal);
        }
        None
    }

    fn admit_next_request(&mut self, policy: &RpcQueuePolicy) -> Option<RequestId> {
        let principal = self.next_admissible_principal(policy)?;
        let (request_id, principal_queue_empty) = {
            let queue = self.pending_by_principal.get_mut(&principal)?;
            let request_id = queue.pop_front()?;
            (request_id, queue.is_empty())
        };

        self.pending_total = self.pending_total.saturating_sub(1);
        if let Some(count) = self.pending_count_by_principal.get_mut(&principal) {
            if *count <= 1 {
                *count = 0;
            } else {
                *count -= 1;
            }
        }
        if principal_queue_empty {
            self.clear_principal_if_idle(&principal);
        }

        self.admitted_requests.insert(request_id);
        self.admitted_total = self.admitted_total.saturating_add(1);
        Some(request_id)
    }

    fn promote_admissions(&mut self, policy: &RpcQueuePolicy, global_limit: usize) -> usize {
        let mut granted = 0usize;
        while self.pending_total > 0 && self.has_global_capacity(global_limit) {
            if self.admit_next_request(policy).is_none() {
                break;
            }
            granted = granted.saturating_add(1);
        }
        granted
    }

    fn take_admitted(&mut self, request_id: RequestId) -> bool {
        self.admitted_requests.remove(&request_id)
    }

    fn remove_pending_request(&mut self, principal: &str, request_id: RequestId) -> bool {
        let mut removed = false;
        let mut principal_queue_empty = false;
        if let Some(queue) = self.pending_by_principal.get_mut(principal) {
            if let Some(index) = queue.iter().position(|candidate| *candidate == request_id) {
                queue.remove(index);
                removed = true;
                principal_queue_empty = queue.is_empty();
            }
        }

        if !removed {
            return false;
        }

        self.pending_total = self.pending_total.saturating_sub(1);
        if let Some(count) = self.pending_count_by_principal.get_mut(principal) {
            if *count <= 1 {
                *count = 0;
            } else {
                *count -= 1;
            }
        }
        if principal_queue_empty {
            self.clear_principal_if_idle(principal);
        }
        true
    }

    fn release_admission(&mut self) {
        self.admitted_total = self.admitted_total.saturating_sub(1);
    }
}

#[derive(Default)]
struct AdmissionQueue {
    state: Mutex<AdmissionState>,
    condvar: Condvar,
}

impl AdmissionQueue {
    fn release_admission(&self) {
        let mut state = self
            .state
            .lock()
            .expect("rpc admission queue mutex should not be poisoned");
        state.release_admission();
        drop(state);
        self.condvar.notify_all();
    }
}

struct QueuePermit {
    queue: &'static AdmissionQueue,
}

impl std::fmt::Debug for QueuePermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueuePermit").finish()
    }
}

impl Drop for QueuePermit {
    fn drop(&mut self) {
        self.queue.release_admission();
    }
}

fn parse_bool(raw: Option<String>, default_value: bool) -> bool {
    let Some(raw) = raw else {
        return default_value;
    };

    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default_value,
    }
}

fn parse_bounded_positive_usize(raw: Option<String>, default_value: usize, max_value: usize) -> usize {
    raw.and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(max_value))
        .unwrap_or(default_value.min(max_value))
}

fn parse_queue_pending_limit(raw: Option<String>, default_limit: usize) -> usize {
    parse_bounded_positive_usize(raw, default_limit, MAX_RPC_QUEUE_PENDING)
}

fn normalize_queue_pending_per_principal_limit(
    max_pending: usize,
    configured_limit: usize,
) -> usize {
    if max_pending == 0 {
        return 0;
    }
    configured_limit.min(max_pending)
}

fn parse_queue_drr_quantum(raw: Option<String>) -> usize {
    parse_bounded_positive_usize(raw, DEFAULT_RPC_QUEUE_DRR_QUANTUM, MAX_RPC_QUEUE_DRR_QUANTUM)
}

fn parse_principal_default_weight(raw: Option<String>) -> usize {
    parse_bounded_positive_usize(
        raw,
        DEFAULT_RPC_PRINCIPAL_WEIGHT_DEFAULT,
        MAX_RPC_PRINCIPAL_WEIGHT,
    )
}

fn parse_principal_weight_map(raw: Option<String>) -> HashMap<String, usize> {
    let mut parsed = HashMap::new();
    let Some(raw) = raw else {
        return parsed;
    };

    for entry in raw.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((principal_raw, weight_raw)) = trimmed.split_once('=') else {
            #[cfg(not(test))]
            eprintln!(
                "SCCP_MCP_RPC_PRINCIPAL_WEIGHTS malformed entry '{trimmed}'; expected principal=weight"
            );
            continue;
        };

        let principal = principal_raw.trim();
        if principal.is_empty() {
            #[cfg(not(test))]
            eprintln!(
                "SCCP_MCP_RPC_PRINCIPAL_WEIGHTS malformed entry '{trimmed}'; principal key is empty"
            );
            continue;
        }

        let Some(weight) = weight_raw
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|weight| *weight > 0)
            .map(|weight| weight.min(MAX_RPC_PRINCIPAL_WEIGHT))
        else {
            #[cfg(not(test))]
            eprintln!(
                "SCCP_MCP_RPC_PRINCIPAL_WEIGHTS malformed entry '{trimmed}'; weight must be a positive integer"
            );
            continue;
        };

        parsed.insert(principal.to_owned(), weight);
    }

    parsed
}

fn rpc_queue_policy() -> RpcQueuePolicy {
    let enabled = parse_bool(
        std::env::var("SCCP_MCP_RPC_QUEUE_ENABLE").ok(),
        DEFAULT_RPC_QUEUE_ENABLE,
    );
    let max_pending = parse_queue_pending_limit(
        std::env::var("SCCP_MCP_RPC_QUEUE_MAX_PENDING").ok(),
        DEFAULT_RPC_QUEUE_MAX_PENDING,
    );
    let configured_max_pending_per_principal = parse_queue_pending_limit(
        std::env::var("SCCP_MCP_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL").ok(),
        DEFAULT_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL,
    );
    let max_pending_per_principal =
        normalize_queue_pending_per_principal_limit(max_pending, configured_max_pending_per_principal);
    let wait_timeout = parse_timeout_ms(
        std::env::var("SCCP_MCP_RPC_QUEUE_WAIT_TIMEOUT_MS").ok(),
        DEFAULT_RPC_QUEUE_WAIT_TIMEOUT_MS,
    );
    let drr_quantum = parse_queue_drr_quantum(std::env::var("SCCP_MCP_RPC_QUEUE_DRR_QUANTUM").ok());
    let principal_weight_default = parse_principal_default_weight(
        std::env::var("SCCP_MCP_RPC_PRINCIPAL_WEIGHT_DEFAULT").ok(),
    );
    let principal_weights =
        parse_principal_weight_map(std::env::var("SCCP_MCP_RPC_PRINCIPAL_WEIGHTS").ok());

    RpcQueuePolicy {
        enabled,
        max_pending,
        max_pending_per_principal,
        wait_timeout,
        drr_quantum,
        principal_weight_default,
        principal_weights,
    }
}

fn try_acquire_rpc_queue_permit(
    principal_key: &str,
    url: &str,
    method: &str,
    global_limit: usize,
    queue_policy: &RpcQueuePolicy,
) -> AppResult<Option<QueuePermit>> {
    if !queue_policy.enabled {
        return Ok(None);
    }

    let queue = &RPC_ADMISSION_QUEUE;
    let start = Instant::now();
    let mut state = queue
        .state
        .lock()
        .expect("rpc admission queue mutex should not be poisoned");

    if state.pending_total == 0 && state.has_global_capacity(global_limit) {
        state.admitted_total = state.admitted_total.saturating_add(1);
        drop(state);
        #[cfg(not(test))]
        eprintln!(
            "SECURITY_RPC_QUEUE_ADMIT principal={principal_key} waited_ms=0 weight={} url={url} method={method}",
            queue_policy.weight_for(principal_key)
        );
        return Ok(Some(QueuePermit { queue }));
    }

    if state.pending_total >= queue_policy.max_pending {
        #[cfg(not(test))]
        let pending_total = state.pending_total;
        drop(state);
        #[cfg(not(test))]
        eprintln!(
            "SECURITY_RPC_QUEUE_BACKPRESSURE reason=queue_full_global principal={principal_key} url={url} method={method} pending_total={pending_total} queue_max_pending={}",
            queue_policy.max_pending
        );
        return Err(AppError::Rpc(format!(
            "rpc admission queue full for {url} method {method}; global pending limit ({}) reached",
            queue_policy.max_pending
        )));
    }

    let pending_for_principal = state.pending_for_principal(principal_key);
    if pending_for_principal >= queue_policy.max_pending_per_principal {
        drop(state);
        #[cfg(not(test))]
        eprintln!(
            "SECURITY_RPC_QUEUE_BACKPRESSURE reason=queue_full_principal principal={principal_key} url={url} method={method} principal_pending={pending_for_principal} queue_max_pending_per_principal={}",
            queue_policy.max_pending_per_principal
        );
        return Err(AppError::Rpc(format!(
            "rpc admission queue full for principal {principal_key} on {url} method {method}; per-principal pending limit ({}) reached",
            queue_policy.max_pending_per_principal
        )));
    }

    let request_id = state.allocate_request_id();
    state.enqueue_request(principal_key, request_id);

    loop {
        if state.promote_admissions(queue_policy, global_limit) > 0 {
            queue.condvar.notify_all();
        }

        if state.take_admitted(request_id) {
            #[cfg(not(test))]
            let waited_ms = start.elapsed().as_millis();
            drop(state);
            #[cfg(not(test))]
            eprintln!(
                "SECURITY_RPC_QUEUE_ADMIT principal={principal_key} waited_ms={waited_ms} weight={} url={url} method={method}",
                queue_policy.weight_for(principal_key)
            );
            return Ok(Some(QueuePermit { queue }));
        }

        let elapsed = start.elapsed();
        if elapsed >= queue_policy.wait_timeout {
            let removed = state.remove_pending_request(principal_key, request_id);
            if removed {
                queue.condvar.notify_all();
            }
            drop(state);
            #[cfg(not(test))]
            let waited_ms = elapsed.as_millis();
            #[cfg(not(test))]
            eprintln!(
                "SECURITY_RPC_QUEUE_BACKPRESSURE reason=queue_timeout principal={principal_key} url={url} method={method} waited_ms={waited_ms} queue_wait_timeout_ms={}",
                queue_policy.wait_timeout.as_millis()
            );
            return Err(AppError::Rpc(format!(
                "rpc admission queue timeout after {} ms for principal {principal_key} on {url} method {method}",
                queue_policy.wait_timeout.as_millis()
            )));
        }

        let remaining = queue_policy.wait_timeout.saturating_sub(elapsed);
        let (next_state, _) = queue
            .condvar
            .wait_timeout(state, remaining)
            .expect("rpc admission queue mutex should not be poisoned");
        state = next_state;
    }
}

fn parse_timeout_ms(raw: Option<String>, default_ms: u64) -> Duration {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(default_ms))
}

fn parse_retry_count(raw: Option<String>, default_count: u32) -> u32 {
    raw.and_then(|value| value.parse::<u32>().ok())
        .map(|value| value.min(10))
        .unwrap_or(default_count)
}

fn parse_backoff_ms(raw: Option<String>, default_ms: u64) -> Duration {
    raw.and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(default_ms))
}

fn parse_failure_threshold(raw: Option<String>, default_threshold: u32) -> u32 {
    raw.and_then(|value| value.parse::<u32>().ok())
        .map(|value| value.min(100))
        .unwrap_or(default_threshold)
}

fn parse_inflight_limit(raw: Option<String>, default_limit: usize) -> usize {
    raw.and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(MAX_RPC_MAX_INFLIGHT))
        .unwrap_or(default_limit)
}

fn rpc_max_inflight_limit() -> usize {
    parse_inflight_limit(
        std::env::var("SCCP_MCP_RPC_MAX_INFLIGHT").ok(),
        DEFAULT_RPC_MAX_INFLIGHT,
    )
}

struct RpcInflightGuard;

impl Drop for RpcInflightGuard {
    fn drop(&mut self) {
        RPC_INFLIGHT.fetch_sub(1, Ordering::AcqRel);
    }
}

fn try_acquire_rpc_inflight_slot(limit: usize) -> Option<RpcInflightGuard> {
    if limit == 0 {
        return None;
    }
    loop {
        let current = RPC_INFLIGHT.load(Ordering::Acquire);
        if current >= limit {
            return None;
        }
        let next = current.checked_add(1)?;
        if RPC_INFLIGHT
            .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return Some(RpcInflightGuard);
        }
    }
}

fn normalize_per_endpoint_inflight_limit(global_limit: usize, configured_limit: usize) -> usize {
    if global_limit == 0 {
        return 0;
    }
    configured_limit.min(global_limit)
}

fn rpc_max_inflight_per_endpoint_limit(global_limit: usize) -> usize {
    let configured_limit = parse_inflight_limit(
        std::env::var("SCCP_MCP_RPC_MAX_INFLIGHT_PER_ENDPOINT").ok(),
        DEFAULT_RPC_MAX_INFLIGHT_PER_ENDPOINT,
    );
    normalize_per_endpoint_inflight_limit(global_limit, configured_limit)
}

fn normalize_per_principal_inflight_limit(global_limit: usize, configured_limit: usize) -> usize {
    if global_limit == 0 {
        return 0;
    }
    configured_limit.min(global_limit)
}

fn rpc_max_inflight_per_principal_limit(global_limit: usize) -> usize {
    let configured_limit = parse_inflight_limit(
        std::env::var("SCCP_MCP_RPC_MAX_INFLIGHT_PER_PRINCIPAL").ok(),
        DEFAULT_RPC_MAX_INFLIGHT_PER_PRINCIPAL,
    );
    normalize_per_principal_inflight_limit(global_limit, configured_limit)
}

fn normalize_per_scope_inflight_limit(global_limit: usize, configured_limit: usize) -> usize {
    if global_limit == 0 {
        return 0;
    }
    configured_limit.min(global_limit)
}

fn rpc_max_inflight_per_scope_limit(global_limit: usize) -> usize {
    let configured_limit = parse_inflight_limit(
        std::env::var("SCCP_MCP_RPC_MAX_INFLIGHT_PER_SCOPE").ok(),
        DEFAULT_RPC_MAX_INFLIGHT_PER_SCOPE,
    );
    normalize_per_scope_inflight_limit(global_limit, configured_limit)
}

fn normalize_per_method_inflight_limit(
    global_limit: usize,
    endpoint_limit: usize,
    configured_limit: usize,
) -> usize {
    if global_limit == 0 || endpoint_limit == 0 {
        return 0;
    }
    configured_limit.min(endpoint_limit).min(global_limit)
}

fn rpc_max_inflight_per_method_limit(global_limit: usize, endpoint_limit: usize) -> usize {
    let configured_limit = parse_inflight_limit(
        std::env::var("SCCP_MCP_RPC_MAX_INFLIGHT_PER_METHOD").ok(),
        DEFAULT_RPC_MAX_INFLIGHT_PER_METHOD,
    );
    normalize_per_method_inflight_limit(global_limit, endpoint_limit, configured_limit)
}

fn current_rpc_fairness_principal() -> Option<String> {
    RPC_FAIRNESS_PRINCIPAL.with(|principal| principal.borrow().clone())
}

pub fn with_rpc_fairness_principal<T>(principal: &str, f: impl FnOnce() -> T) -> T {
    RPC_FAIRNESS_PRINCIPAL.with(|current| {
        let previous = current.replace(Some(principal.to_owned()));
        let output = f();
        current.replace(previous);
        output
    })
}

fn current_rpc_fairness_scope() -> Option<String> {
    RPC_FAIRNESS_SCOPE.with(|scope| scope.borrow().clone())
}

pub fn with_rpc_fairness_scope<T>(scope: &str, f: impl FnOnce() -> T) -> T {
    RPC_FAIRNESS_SCOPE.with(|current| {
        let previous = current.replace(Some(scope.to_owned()));
        let output = f();
        current.replace(previous);
        output
    })
}

struct RpcEndpointInflightGuard {
    endpoint_key: String,
}

impl Drop for RpcEndpointInflightGuard {
    fn drop(&mut self) {
        let mut by_endpoint = RPC_INFLIGHT_BY_ENDPOINT
            .lock()
            .expect("endpoint in-flight mutex should not be poisoned");
        if let Some(count) = by_endpoint.get_mut(&self.endpoint_key) {
            if *count <= 1 {
                by_endpoint.remove(&self.endpoint_key);
            } else {
                *count -= 1;
            }
        }
    }
}

fn try_acquire_rpc_endpoint_inflight_slot(
    endpoint_key: &str,
    limit: usize,
) -> Option<RpcEndpointInflightGuard> {
    if limit == 0 {
        return None;
    }

    let mut by_endpoint = RPC_INFLIGHT_BY_ENDPOINT
        .lock()
        .expect("endpoint in-flight mutex should not be poisoned");
    let current = *by_endpoint.get(endpoint_key).unwrap_or(&0);
    if current >= limit {
        return None;
    }

    let next = current.checked_add(1)?;
    by_endpoint.insert(endpoint_key.to_owned(), next);
    Some(RpcEndpointInflightGuard {
        endpoint_key: endpoint_key.to_owned(),
    })
}

struct RpcPrincipalInflightGuard {
    principal_key: String,
}

impl Drop for RpcPrincipalInflightGuard {
    fn drop(&mut self) {
        let mut by_principal = RPC_INFLIGHT_BY_PRINCIPAL
            .lock()
            .expect("principal in-flight mutex should not be poisoned");
        if let Some(count) = by_principal.get_mut(&self.principal_key) {
            if *count <= 1 {
                by_principal.remove(&self.principal_key);
            } else {
                *count -= 1;
            }
        }
    }
}

fn try_acquire_rpc_principal_inflight_slot(
    principal_key: &str,
    limit: usize,
) -> Option<RpcPrincipalInflightGuard> {
    if limit == 0 {
        return None;
    }

    let mut by_principal = RPC_INFLIGHT_BY_PRINCIPAL
        .lock()
        .expect("principal in-flight mutex should not be poisoned");
    let current = *by_principal.get(principal_key).unwrap_or(&0);
    if current >= limit {
        return None;
    }

    let next = current.checked_add(1)?;
    by_principal.insert(principal_key.to_owned(), next);
    Some(RpcPrincipalInflightGuard {
        principal_key: principal_key.to_owned(),
    })
}

struct RpcScopeInflightGuard {
    scope_key: String,
}

impl Drop for RpcScopeInflightGuard {
    fn drop(&mut self) {
        let mut by_scope = RPC_INFLIGHT_BY_SCOPE
            .lock()
            .expect("scope in-flight mutex should not be poisoned");
        if let Some(count) = by_scope.get_mut(&self.scope_key) {
            if *count <= 1 {
                by_scope.remove(&self.scope_key);
            } else {
                *count -= 1;
            }
        }
    }
}

fn try_acquire_rpc_scope_inflight_slot(
    scope_key: &str,
    limit: usize,
) -> Option<RpcScopeInflightGuard> {
    if limit == 0 {
        return None;
    }

    let mut by_scope = RPC_INFLIGHT_BY_SCOPE
        .lock()
        .expect("scope in-flight mutex should not be poisoned");
    let current = *by_scope.get(scope_key).unwrap_or(&0);
    if current >= limit {
        return None;
    }

    let next = current.checked_add(1)?;
    by_scope.insert(scope_key.to_owned(), next);
    Some(RpcScopeInflightGuard {
        scope_key: scope_key.to_owned(),
    })
}

struct RpcMethodInflightGuard {
    method_key: String,
}

impl Drop for RpcMethodInflightGuard {
    fn drop(&mut self) {
        let mut by_method = RPC_INFLIGHT_BY_METHOD
            .lock()
            .expect("method in-flight mutex should not be poisoned");
        if let Some(count) = by_method.get_mut(&self.method_key) {
            if *count <= 1 {
                by_method.remove(&self.method_key);
            } else {
                *count -= 1;
            }
        }
    }
}

fn try_acquire_rpc_method_inflight_slot(
    method_key: &str,
    limit: usize,
) -> Option<RpcMethodInflightGuard> {
    if limit == 0 {
        return None;
    }

    let mut by_method = RPC_INFLIGHT_BY_METHOD
        .lock()
        .expect("method in-flight mutex should not be poisoned");
    let current = *by_method.get(method_key).unwrap_or(&0);
    if current >= limit {
        return None;
    }

    let next = current.checked_add(1)?;
    by_method.insert(method_key.to_owned(), next);
    Some(RpcMethodInflightGuard {
        method_key: method_key.to_owned(),
    })
}

fn rpc_agent() -> ureq::Agent {
    let connect_timeout = parse_timeout_ms(
        std::env::var("SCCP_MCP_RPC_CONNECT_TIMEOUT_MS").ok(),
        DEFAULT_RPC_CONNECT_TIMEOUT_MS,
    );
    let io_timeout = parse_timeout_ms(
        std::env::var("SCCP_MCP_RPC_IO_TIMEOUT_MS").ok(),
        DEFAULT_RPC_IO_TIMEOUT_MS,
    );

    ureq::AgentBuilder::new()
        .timeout_connect(connect_timeout)
        .timeout_read(io_timeout)
        .timeout_write(io_timeout)
        .build()
}

fn rpc_retry_policy() -> RetryPolicy {
    RetryPolicy {
        max_retries: parse_retry_count(
            std::env::var("SCCP_MCP_RPC_MAX_RETRIES").ok(),
            DEFAULT_RPC_MAX_RETRIES,
        ),
        backoff: parse_backoff_ms(
            std::env::var("SCCP_MCP_RPC_RETRY_BACKOFF_MS").ok(),
            DEFAULT_RPC_RETRY_BACKOFF_MS,
        ),
    }
}

fn rpc_circuit_breaker_policy() -> CircuitBreakerPolicy {
    CircuitBreakerPolicy {
        failure_threshold: parse_failure_threshold(
            std::env::var("SCCP_MCP_RPC_CIRCUIT_BREAKER_THRESHOLD").ok(),
            DEFAULT_RPC_CIRCUIT_BREAKER_THRESHOLD,
        ),
        cooldown: parse_backoff_ms(
            std::env::var("SCCP_MCP_RPC_CIRCUIT_BREAKER_COOLDOWN_MS").ok(),
            DEFAULT_RPC_CIRCUIT_BREAKER_COOLDOWN_MS,
        ),
    }
}

fn circuit_states() -> &'static Mutex<HashMap<String, CircuitState>> {
    &CIRCUIT_STATES
}

fn circuit_key(url: &str) -> String {
    url.to_owned()
}

fn circuit_open_remaining(key: &str, policy: CircuitBreakerPolicy) -> Option<Duration> {
    if policy.failure_threshold == 0 {
        return None;
    }
    let now = Instant::now();
    let mut states = circuit_states()
        .lock()
        .expect("circuit state mutex should not be poisoned");
    let state = states.entry(key.to_owned()).or_default();
    if let Some(until) = state.open_until {
        if until > now {
            return Some(until.saturating_duration_since(now));
        }
        state.open_until = None;
        state.consecutive_failures = 0;
    }
    None
}

fn record_circuit_success(key: &str) {
    let mut states = circuit_states()
        .lock()
        .expect("circuit state mutex should not be poisoned");
    states.remove(key);
}

fn record_circuit_failure(key: &str, policy: CircuitBreakerPolicy) {
    if policy.failure_threshold == 0 {
        return;
    }

    let now = Instant::now();
    let mut states = circuit_states()
        .lock()
        .expect("circuit state mutex should not be poisoned");
    let state = states.entry(key.to_owned()).or_default();

    if let Some(until) = state.open_until {
        if until > now {
            return;
        }
        state.open_until = None;
        state.consecutive_failures = 0;
    }

    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    if state.consecutive_failures >= policy.failure_threshold {
        state.open_until = Some(now + policy.cooldown);
    }
}

fn method_is_retry_safe(method: &str) -> bool {
    let lower = method.to_ascii_lowercase();
    !(lower.starts_with("author_submit")
        || lower.contains("sendrawtransaction")
        || lower.starts_with("send")
        || lower.contains("submit"))
}

fn is_retryable_http_status(code: u16) -> bool {
    matches!(code, 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

fn retry_delay(backoff: Duration, attempt: u32) -> Duration {
    let base_ms = backoff.as_millis().min(u128::from(u64::MAX)) as u64;
    let multiplier = u64::from(attempt).saturating_add(1);
    Duration::from_millis(base_ms.saturating_mul(multiplier))
}

pub fn rpc_call(url: &str, method: &str, params: Value) -> AppResult<Value> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let payload = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });

    let retry_policy = rpc_retry_policy();
    let circuit_policy = rpc_circuit_breaker_policy();
    let retry_safe = method_is_retry_safe(method);
    let agent = rpc_agent();
    let key = circuit_key(url);
    let global_inflight_limit = rpc_max_inflight_limit();
    let endpoint_inflight_limit = rpc_max_inflight_per_endpoint_limit(global_inflight_limit);
    let principal_inflight_limit = rpc_max_inflight_per_principal_limit(global_inflight_limit);
    let scope_inflight_limit = rpc_max_inflight_per_scope_limit(global_inflight_limit);
    let method_inflight_limit =
        rpc_max_inflight_per_method_limit(global_inflight_limit, endpoint_inflight_limit);
    let fairness_principal =
        current_rpc_fairness_principal().unwrap_or_else(|| "anonymous".to_owned());
    let fairness_scope = current_rpc_fairness_scope().unwrap_or_else(|| "unspecified".to_owned());
    let queue_policy = rpc_queue_policy();
    let _queue_permit = try_acquire_rpc_queue_permit(
        &fairness_principal,
        url,
        method,
        global_inflight_limit,
        &queue_policy,
    )?;
    let _inflight_guard =
        try_acquire_rpc_inflight_slot(global_inflight_limit).ok_or_else(|| {
            #[cfg(not(test))]
            eprintln!(
                "SECURITY_RPC_BACKPRESSURE scope=global url={url} method={method} in_flight_limit={global_inflight_limit}"
            );
            AppError::Rpc(format!(
                "rpc in-flight limit reached ({global_inflight_limit}) for {url} method {method}; reject to preserve availability"
            ))
        })?;
    let _endpoint_inflight_guard =
        try_acquire_rpc_endpoint_inflight_slot(&key, endpoint_inflight_limit).ok_or_else(|| {
            #[cfg(not(test))]
            eprintln!(
                "SECURITY_RPC_BACKPRESSURE scope=endpoint url={url} method={method} endpoint_in_flight_limit={endpoint_inflight_limit} global_in_flight_limit={global_inflight_limit}"
            );
            AppError::Rpc(format!(
                "rpc endpoint in-flight limit reached ({endpoint_inflight_limit}) for {url} method {method}; reject to preserve availability"
            ))
        })?;
    let _principal_inflight_guard =
        try_acquire_rpc_principal_inflight_slot(&fairness_principal, principal_inflight_limit)
            .ok_or_else(|| {
                #[cfg(not(test))]
                eprintln!(
                    "SECURITY_RPC_BACKPRESSURE scope=principal principal={fairness_principal} url={url} method={method} principal_in_flight_limit={principal_inflight_limit} endpoint_in_flight_limit={endpoint_inflight_limit} global_in_flight_limit={global_inflight_limit}"
                );
                AppError::Rpc(format!(
                    "rpc principal in-flight limit reached ({principal_inflight_limit}) for principal {fairness_principal} url {url} method {method}; reject to preserve availability"
                ))
            })?;
    let _scope_inflight_guard =
        try_acquire_rpc_scope_inflight_slot(&fairness_scope, scope_inflight_limit).ok_or_else(
            || {
                #[cfg(not(test))]
                eprintln!(
                    "SECURITY_RPC_BACKPRESSURE scope=tool tool={fairness_scope} url={url} method={method} tool_in_flight_limit={scope_inflight_limit} endpoint_in_flight_limit={endpoint_inflight_limit} global_in_flight_limit={global_inflight_limit}"
                );
                AppError::Rpc(format!(
                    "rpc tool in-flight limit reached ({scope_inflight_limit}) for tool {fairness_scope} url {url} method {method}; reject to preserve availability"
                ))
            },
        )?;
    let method_key = format!("{key}::{method}");
    let _method_inflight_guard =
        try_acquire_rpc_method_inflight_slot(&method_key, method_inflight_limit).ok_or_else(
            || {
                #[cfg(not(test))]
                eprintln!(
                    "SECURITY_RPC_BACKPRESSURE scope=method url={url} method={method} method_in_flight_limit={method_inflight_limit} endpoint_in_flight_limit={endpoint_inflight_limit} global_in_flight_limit={global_inflight_limit}"
                );
                AppError::Rpc(format!(
                    "rpc method in-flight limit reached ({method_inflight_limit}) for {url} method {method}; reject to preserve availability"
                ))
            },
        )?;

    if let Some(remaining) = circuit_open_remaining(&key, circuit_policy) {
        return Err(AppError::Rpc(format!(
            "rpc circuit breaker open for {url} (method {method}); retry after {} ms",
            remaining.as_millis()
        )));
    }

    for attempt in 0..=retry_policy.max_retries {
        let response = agent
            .post(url)
            .set("content-type", "application/json")
            .send_json(payload.clone());

        match response {
            Ok(response) => {
                let body: Value = response.into_json().map_err(|err| {
                    AppError::Rpc(format!("rpc response from {url} was not valid JSON: {err}"))
                })?;

                if let Some(err) = body.get("error") {
                    return Err(AppError::Rpc(format!(
                        "rpc error from {url} method {method}: {err}"
                    )));
                }

                record_circuit_success(&key);
                return body.get("result").cloned().ok_or_else(|| {
                    AppError::Rpc(format!("rpc response from {url} missing result field"))
                });
            }
            Err(ureq::Error::Status(code, response)) => {
                let retryable_status = is_retryable_http_status(code);
                let can_retry =
                    retry_safe && attempt < retry_policy.max_retries && retryable_status;
                if can_retry {
                    thread::sleep(retry_delay(retry_policy.backoff, attempt));
                    continue;
                }
                if retryable_status {
                    record_circuit_failure(&key, circuit_policy);
                }
                let status_text = response.status_text().to_owned();
                let body_text = response
                    .into_string()
                    .unwrap_or_else(|_| String::from("<unavailable>"));
                return Err(AppError::Rpc(format!(
                    "rpc request to {url} method {method} failed with HTTP {code} ({status_text}): {body_text}"
                )));
            }
            Err(ureq::Error::Transport(err)) => {
                let can_retry = retry_safe && attempt < retry_policy.max_retries;
                if can_retry {
                    thread::sleep(retry_delay(retry_policy.backoff, attempt));
                    continue;
                }
                record_circuit_failure(&key, circuit_policy);
                return Err(AppError::Rpc(format!(
                    "rpc request to {url} method {method} failed: {err}"
                )));
            }
        }
    }

    Err(AppError::Rpc(format!(
        "rpc request to {url} method {method} failed unexpectedly"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    static QUEUE_TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn test_queue_policy() -> RpcQueuePolicy {
        RpcQueuePolicy {
            enabled: true,
            max_pending: DEFAULT_RPC_QUEUE_MAX_PENDING,
            max_pending_per_principal: DEFAULT_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL,
            wait_timeout: Duration::from_millis(DEFAULT_RPC_QUEUE_WAIT_TIMEOUT_MS),
            drr_quantum: DEFAULT_RPC_QUEUE_DRR_QUANTUM,
            principal_weight_default: DEFAULT_RPC_PRINCIPAL_WEIGHT_DEFAULT,
            principal_weights: HashMap::new(),
        }
    }

    fn reset_rpc_admission_queue_state() {
        let mut state = RPC_ADMISSION_QUEUE
            .state
            .lock()
            .expect("rpc admission queue mutex should not be poisoned");
        *state = AdmissionState::default();
        drop(state);
        RPC_ADMISSION_QUEUE.condvar.notify_all();
    }

    fn with_env_overrides(overrides: &[(&str, Option<&str>)], f: impl FnOnce()) {
        let mut previous = Vec::with_capacity(overrides.len());
        for (key, value) in overrides {
            previous.push(((*key).to_owned(), std::env::var(key).ok()));
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }

        f();

        for (key, value) in previous {
            match value {
                Some(value) => std::env::set_var(&key, value),
                None => std::env::remove_var(&key),
            }
        }
    }

    #[test]
    fn queue_disabled_path_is_noop() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.enabled = false;

        let permit = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy)
            .expect("queue-disabled path should not error");
        assert!(permit.is_none(), "queue-disabled path should not allocate permit");

        let state = RPC_ADMISSION_QUEUE
            .state
            .lock()
            .expect("rpc admission queue mutex should not be poisoned");
        assert_eq!(state.pending_total, 0);
        assert_eq!(state.admitted_total, 0);
    }

    #[test]
    fn queue_immediate_admission_updates_and_releases_counters() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let policy = test_queue_policy();

        let permit = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 2, &policy)
            .expect("immediate admission should succeed")
            .expect("queue should return permit");
        {
            let state = RPC_ADMISSION_QUEUE
                .state
                .lock()
                .expect("rpc admission queue mutex should not be poisoned");
            assert_eq!(state.pending_total, 0);
            assert_eq!(state.admitted_total, 1);
        }

        drop(permit);
        let state = RPC_ADMISSION_QUEUE
            .state
            .lock()
            .expect("rpc admission queue mutex should not be poisoned");
        assert_eq!(state.admitted_total, 0, "dropping permit should release admission slot");
    }

    #[test]
    fn drr_equal_weights_alternate_between_principals() {
        let mut state = AdmissionState::default();
        let policy = test_queue_policy();
        let mut owner_by_request_id = HashMap::<RequestId, String>::new();

        for idx in 0..8u64 {
            let a_id = idx + 1;
            let b_id = idx + 100;
            state.enqueue_request("principal-a", a_id);
            state.enqueue_request("principal-b", b_id);
            owner_by_request_id.insert(a_id, "principal-a".to_owned());
            owner_by_request_id.insert(b_id, "principal-b".to_owned());
        }

        let mut order = Vec::<String>::new();
        for _ in 0..8 {
            let request_id = state
                .admit_next_request(&policy)
                .expect("admission should be available");
            order.push(
                owner_by_request_id
                    .get(&request_id)
                    .expect("request owner should be tracked")
                    .clone(),
            );
            assert!(
                state.take_admitted(request_id),
                "admitted request should be visible to waiter"
            );
            state.release_admission();
        }

        assert_eq!(
            order,
            vec![
                "principal-a".to_owned(),
                "principal-b".to_owned(),
                "principal-a".to_owned(),
                "principal-b".to_owned(),
                "principal-a".to_owned(),
                "principal-b".to_owned(),
                "principal-a".to_owned(),
                "principal-b".to_owned()
            ]
        );
    }

    #[test]
    fn drr_quantum_batches_admissions_per_principal() {
        let mut state = AdmissionState::default();
        let mut policy = test_queue_policy();
        policy.drr_quantum = 2;
        let mut owner_by_request_id = HashMap::<RequestId, String>::new();

        for idx in 0..8u64 {
            let a_id = idx + 1;
            let b_id = idx + 100;
            state.enqueue_request("principal-a", a_id);
            state.enqueue_request("principal-b", b_id);
            owner_by_request_id.insert(a_id, "principal-a".to_owned());
            owner_by_request_id.insert(b_id, "principal-b".to_owned());
        }

        let mut order = Vec::<String>::new();
        for _ in 0..8 {
            let request_id = state
                .admit_next_request(&policy)
                .expect("admission should be available");
            order.push(
                owner_by_request_id
                    .get(&request_id)
                    .expect("request owner should be tracked")
                    .clone(),
            );
            assert!(
                state.take_admitted(request_id),
                "admitted request should be visible to waiter"
            );
            state.release_admission();
        }

        assert_eq!(
            order,
            vec![
                "principal-a".to_owned(),
                "principal-a".to_owned(),
                "principal-b".to_owned(),
                "principal-b".to_owned(),
                "principal-a".to_owned(),
                "principal-a".to_owned(),
                "principal-b".to_owned(),
                "principal-b".to_owned()
            ]
        );
    }

    #[test]
    fn drr_weighted_fairness_prefers_higher_weight_principal() {
        let mut state = AdmissionState::default();
        let mut policy = test_queue_policy();
        policy.principal_weights.insert("principal-heavy".to_owned(), 3);
        policy.principal_weights.insert("principal-light".to_owned(), 1);
        let mut owner_by_request_id = HashMap::<RequestId, String>::new();

        for idx in 0..600u64 {
            let heavy_id = idx + 1;
            let light_id = idx + 1_000;
            state.enqueue_request("principal-heavy", heavy_id);
            state.enqueue_request("principal-light", light_id);
            owner_by_request_id.insert(heavy_id, "principal-heavy".to_owned());
            owner_by_request_id.insert(light_id, "principal-light".to_owned());
        }

        let mut heavy = 0usize;
        let mut light = 0usize;
        for _ in 0..300usize {
            let request_id = state
                .admit_next_request(&policy)
                .expect("admission should be available");
            match owner_by_request_id.get(&request_id).map(|principal| principal.as_str()) {
                Some("principal-heavy") => heavy = heavy.saturating_add(1),
                Some("principal-light") => light = light.saturating_add(1),
                _ => panic!("unexpected principal owner"),
            }
            assert!(
                state.take_admitted(request_id),
                "admitted request should be visible to waiter"
            );
            state.release_admission();
        }

        assert!(light > 0, "light principal should receive admissions");
        let ratio = heavy as f64 / light as f64;
        assert!(
            (2.4..=3.6).contains(&ratio),
            "heavy/light ratio should be near configured 3:1, got {ratio}"
        );
    }

    #[test]
    fn admission_promote_respects_global_limit() {
        let mut state = AdmissionState::default();
        let policy = test_queue_policy();

        state.enqueue_request("principal-a", 1);
        state.enqueue_request("principal-b", 2);
        state.enqueue_request("principal-a", 3);
        state.enqueue_request("principal-b", 4);

        let first_granted = state.promote_admissions(&policy, 2);
        assert_eq!(first_granted, 2, "should only grant up to global limit");
        assert_eq!(state.admitted_total, 2, "admitted count should track grants");
        assert_eq!(state.pending_total, 2, "remaining pending should stay queued");

        let second_granted = state.promote_admissions(&policy, 2);
        assert_eq!(
            second_granted, 0,
            "no further grants should occur while at global limit"
        );

        let admitted_request_id = *state
            .admitted_requests
            .iter()
            .next()
            .expect("at least one admitted request should exist");
        assert!(
            state.take_admitted(admitted_request_id),
            "admitted request should be removable by waiter"
        );
        state.release_admission();

        let third_granted = state.promote_admissions(&policy, 2);
        assert_eq!(
            third_granted, 1,
            "a released slot should allow one additional promotion"
        );
    }

    #[test]
    fn queue_enforces_per_principal_pending_cap() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.max_pending = 8;
        policy.max_pending_per_principal = 1;
        policy.wait_timeout = Duration::from_millis(500);

        let permit = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy)
            .expect("first permit should be granted")
            .expect("queue should return permit");

        let policy_for_waiter = policy.clone();
        let waiter = thread::spawn(move || {
            try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy_for_waiter)
        });

        thread::sleep(Duration::from_millis(30));
        let rejected = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy)
            .expect_err("second pending request for same principal should be rejected");
        let rejected_text = format!("{rejected}");
        assert!(
            rejected_text.contains("per-principal pending limit"),
            "unexpected rejection: {rejected_text}"
        );

        drop(permit);
        let waiter_result = waiter.join().expect("waiter thread should join");
        assert!(
            waiter_result
                .expect("waiter should eventually be admitted after release")
                .is_some(),
            "waiter should receive queue permit"
        );
    }

    #[test]
    fn queue_enforces_global_pending_cap() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.max_pending = 1;
        policy.max_pending_per_principal = 1;
        policy.wait_timeout = Duration::from_millis(500);

        let permit = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy)
            .expect("first permit should be granted")
            .expect("queue should return permit");

        let policy_for_waiter = policy.clone();
        let waiter = thread::spawn(move || {
            try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy_for_waiter)
        });

        thread::sleep(Duration::from_millis(30));
        let rejected = try_acquire_rpc_queue_permit("principal-b", "test://rpc", "eth_call", 1, &policy)
            .expect_err("global pending queue should reject additional enqueue");
        let rejected_text = format!("{rejected}");
        assert!(
            rejected_text.contains("global pending limit"),
            "unexpected rejection: {rejected_text}"
        );

        drop(permit);
        let waiter_result = waiter.join().expect("waiter thread should join");
        assert!(
            waiter_result
                .expect("waiter should eventually be admitted after release")
                .is_some(),
            "waiter should receive queue permit"
        );
    }

    #[test]
    fn queue_timeout_returns_explicit_error() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.wait_timeout = Duration::from_millis(40);
        policy.max_pending = 8;
        policy.max_pending_per_principal = 8;

        let permit = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy)
            .expect("first permit should be granted")
            .expect("queue should return permit");

        let timed_out = try_acquire_rpc_queue_permit("principal-b", "test://rpc", "eth_call", 1, &policy)
            .expect_err("waiting request should time out when no capacity frees");
        let timed_out_text = format!("{timed_out}");
        assert!(
            timed_out_text.contains("queue timeout"),
            "unexpected timeout error: {timed_out_text}"
        );

        drop(permit);
    }

    #[test]
    fn queue_timeout_cleans_pending_state() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.wait_timeout = Duration::from_millis(40);
        policy.max_pending = 8;
        policy.max_pending_per_principal = 8;

        let permit =
            try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy)
                .expect("first permit should be granted")
                .expect("queue should return permit");

        let timed_out =
            try_acquire_rpc_queue_permit("principal-b", "test://rpc", "eth_call", 1, &policy)
                .expect_err("second principal should time out while slot is held");
        let timed_out_text = format!("{timed_out}");
        assert!(
            timed_out_text.contains("queue timeout"),
            "unexpected timeout error: {timed_out_text}"
        );

        let state = RPC_ADMISSION_QUEUE
            .state
            .lock()
            .expect("rpc admission queue mutex should not be poisoned");
        assert_eq!(
            state.pending_total, 0,
            "timed-out request should be removed from pending queue"
        );
        assert_eq!(
            state.pending_for_principal("principal-b"),
            0,
            "timed-out principal should not retain pending count"
        );
        drop(state);
        drop(permit);
    }

    #[test]
    fn queue_permit_raii_drop_wakes_waiter() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.wait_timeout = Duration::from_millis(500);
        policy.max_pending = 8;
        policy.max_pending_per_principal = 8;

        let permit = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 1, &policy)
            .expect("first permit should be granted")
            .expect("queue should return permit");

        let policy_for_waiter = policy.clone();
        let waiter = thread::spawn(move || {
            try_acquire_rpc_queue_permit("principal-b", "test://rpc", "eth_call", 1, &policy_for_waiter)
        });

        thread::sleep(Duration::from_millis(30));
        drop(permit);

        let waiter_result = waiter.join().expect("waiter thread should join");
        assert!(
            waiter_result
                .expect("waiter should be admitted after permit drop")
                .is_some(),
            "waiter should receive queue permit"
        );
    }

    #[test]
    fn queue_parser_robustness_falls_back_to_defaults() {
        assert_eq!(parse_bool(None, false), false);
        assert_eq!(parse_bool(Some("true".to_owned()), false), true);
        assert_eq!(parse_bool(Some("1".to_owned()), false), true);
        assert_eq!(parse_bool(Some("off".to_owned()), true), false);
        assert_eq!(parse_bool(Some("bad".to_owned()), true), true);

        assert_eq!(
            parse_queue_pending_limit(Some("0".to_owned()), 256),
            256,
            "zero should fall back to default"
        );
        assert_eq!(
            parse_queue_pending_limit(Some("999999".to_owned()), 256),
            MAX_RPC_QUEUE_PENDING,
            "queue pending should clamp to safe maximum"
        );
        assert_eq!(
            normalize_queue_pending_per_principal_limit(16, 64),
            16,
            "per-principal pending should clamp to global pending"
        );
        assert_eq!(parse_queue_drr_quantum(Some("0".to_owned())), 1);
        assert_eq!(parse_principal_default_weight(Some("0".to_owned())), 1);

        let parsed = parse_principal_weight_map(Some(
            "requester:a=3,bad,requester:b=oops,=2,auth:c=0,auth:d=4".to_owned(),
        ));
        assert_eq!(parsed.get("requester:a"), Some(&3usize));
        assert_eq!(parsed.get("auth:d"), Some(&4usize));
        assert_eq!(parsed.len(), 2, "malformed entries should be ignored");
    }

    #[test]
    fn parse_bool_accepts_expected_literals() {
        assert!(parse_bool(Some("yes".to_owned()), false));
        assert!(parse_bool(Some("On".to_owned()), false));
        assert!(!parse_bool(Some("no".to_owned()), true));
        assert!(!parse_bool(Some("OFF".to_owned()), true));
    }

    #[test]
    fn parse_bounded_positive_usize_clamps_default_and_valid_values() {
        assert_eq!(
            parse_bounded_positive_usize(None, 9999, 128),
            128,
            "default should clamp to max"
        );
        assert_eq!(
            parse_bounded_positive_usize(Some("96".to_owned()), 8, 128),
            96,
            "valid value in range should parse directly"
        );
        assert_eq!(
            parse_bounded_positive_usize(Some("512".to_owned()), 8, 128),
            128,
            "valid value above max should clamp"
        );
    }

    #[test]
    fn queue_parser_clamps_quantum_and_default_weight() {
        assert_eq!(
            parse_queue_drr_quantum(Some("999999".to_owned())),
            MAX_RPC_QUEUE_DRR_QUANTUM,
            "DRR quantum should clamp to safe maximum"
        );
        assert_eq!(
            parse_queue_drr_quantum(Some("0".to_owned())),
            DEFAULT_RPC_QUEUE_DRR_QUANTUM,
            "zero DRR quantum should fall back to default"
        );

        assert_eq!(
            parse_principal_default_weight(Some("999999".to_owned())),
            MAX_RPC_PRINCIPAL_WEIGHT,
            "default principal weight should clamp to safe maximum"
        );
        assert_eq!(
            parse_principal_default_weight(Some("0".to_owned())),
            DEFAULT_RPC_PRINCIPAL_WEIGHT_DEFAULT,
            "zero default principal weight should fall back to default"
        );
    }

    #[test]
    fn queue_parser_uses_defaults_for_invalid_numeric_text() {
        assert_eq!(
            parse_queue_pending_limit(Some("invalid".to_owned()), 123),
            123,
            "invalid pending limit text should fall back to default"
        );
        assert_eq!(
            parse_queue_drr_quantum(Some("invalid".to_owned())),
            DEFAULT_RPC_QUEUE_DRR_QUANTUM,
            "invalid quantum text should fall back to default"
        );
        assert_eq!(
            parse_principal_default_weight(Some("invalid".to_owned())),
            DEFAULT_RPC_PRINCIPAL_WEIGHT_DEFAULT,
            "invalid weight text should fall back to default"
        );
    }

    #[test]
    fn normalize_queue_pending_per_principal_limit_handles_zero_global() {
        assert_eq!(
            normalize_queue_pending_per_principal_limit(0, 32),
            0,
            "per-principal pending should be forced to zero when global cap is zero"
        );
    }

    #[test]
    fn parse_principal_weight_map_trims_whitespace() {
        let parsed = parse_principal_weight_map(Some(
            " requester:abc = 2 , auth:def = 5 ".to_owned(),
        ));
        assert_eq!(parsed.get("requester:abc"), Some(&2usize));
        assert_eq!(parsed.get("auth:def"), Some(&5usize));
    }

    #[test]
    fn queue_weight_lookup_uses_override_or_default() {
        let mut policy = test_queue_policy();
        policy.drr_quantum = 3;
        policy.principal_weight_default = 2;
        policy.principal_weights.insert("principal-a".to_owned(), 5);

        assert_eq!(
            policy.weight_for("principal-a"),
            5,
            "explicit principal weight should override default"
        );
        assert_eq!(
            policy.weight_for("principal-b"),
            2,
            "missing principal should use default weight"
        );
        assert_eq!(
            policy.deficit_increment_for("principal-a"),
            15,
            "deficit increment should be quantum * weight"
        );
        assert_eq!(policy.deficit_increment_for("principal-b"), 6);
    }

    #[test]
    fn rpc_queue_policy_uses_secure_defaults_for_invalid_env() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        with_env_overrides(
            &[
                ("SCCP_MCP_RPC_QUEUE_ENABLE", Some("invalid")),
                ("SCCP_MCP_RPC_QUEUE_MAX_PENDING", Some("0")),
                ("SCCP_MCP_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL", Some("0")),
                ("SCCP_MCP_RPC_QUEUE_WAIT_TIMEOUT_MS", Some("0")),
                ("SCCP_MCP_RPC_QUEUE_DRR_QUANTUM", Some("0")),
                ("SCCP_MCP_RPC_PRINCIPAL_WEIGHT_DEFAULT", Some("0")),
                (
                    "SCCP_MCP_RPC_PRINCIPAL_WEIGHTS",
                    Some("bad,requester:a=oops"),
                ),
            ],
            || {
                let policy = rpc_queue_policy();
                assert!(
                    !policy.enabled,
                    "invalid bool should fall back to default disabled"
                );
                assert_eq!(policy.max_pending, DEFAULT_RPC_QUEUE_MAX_PENDING);
                assert_eq!(
                    policy.max_pending_per_principal,
                    DEFAULT_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL
                );
                assert_eq!(
                    policy.wait_timeout,
                    Duration::from_millis(DEFAULT_RPC_QUEUE_WAIT_TIMEOUT_MS)
                );
                assert_eq!(policy.drr_quantum, DEFAULT_RPC_QUEUE_DRR_QUANTUM);
                assert_eq!(
                    policy.principal_weight_default,
                    DEFAULT_RPC_PRINCIPAL_WEIGHT_DEFAULT
                );
                assert!(
                    policy.principal_weights.is_empty(),
                    "invalid mapping entries should be ignored"
                );
            },
        );
    }

    #[test]
    fn rpc_queue_policy_parses_and_normalizes_valid_env() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        with_env_overrides(
            &[
                ("SCCP_MCP_RPC_QUEUE_ENABLE", Some("true")),
                ("SCCP_MCP_RPC_QUEUE_MAX_PENDING", Some("5000")),
                ("SCCP_MCP_RPC_QUEUE_MAX_PENDING_PER_PRINCIPAL", Some("4500")),
                ("SCCP_MCP_RPC_QUEUE_WAIT_TIMEOUT_MS", Some("750")),
                ("SCCP_MCP_RPC_QUEUE_DRR_QUANTUM", Some("4")),
                ("SCCP_MCP_RPC_PRINCIPAL_WEIGHT_DEFAULT", Some("7")),
                (
                    "SCCP_MCP_RPC_PRINCIPAL_WEIGHTS",
                    Some("requester:a=3,auth:b=999999"),
                ),
            ],
            || {
                let policy = rpc_queue_policy();
                assert!(policy.enabled);
                assert_eq!(
                    policy.max_pending, MAX_RPC_QUEUE_PENDING,
                    "max pending should clamp to safe upper bound"
                );
                assert_eq!(
                    policy.max_pending_per_principal, MAX_RPC_QUEUE_PENDING,
                    "per-principal pending should clamp to global pending cap"
                );
                assert_eq!(policy.wait_timeout, Duration::from_millis(750));
                assert_eq!(policy.drr_quantum, 4);
                assert_eq!(policy.principal_weight_default, 7);
                assert_eq!(policy.principal_weights.get("requester:a"), Some(&3usize));
                assert_eq!(
                    policy.principal_weights.get("auth:b"),
                    Some(&MAX_RPC_PRINCIPAL_WEIGHT)
                );
            },
        );
    }

    #[test]
    fn parse_principal_weight_map_duplicate_entries_last_wins_and_clamps() {
        let parsed = parse_principal_weight_map(Some(
            "requester:a=2,requester:a=9,auth:b=999999".to_owned(),
        ));
        assert_eq!(
            parsed.get("requester:a"),
            Some(&9usize),
            "later duplicate should override earlier entry"
        );
        assert_eq!(
            parsed.get("auth:b"),
            Some(&MAX_RPC_PRINCIPAL_WEIGHT),
            "oversized weight should clamp to safe maximum"
        );
    }

    #[test]
    fn queue_does_not_bypass_existing_pending_when_capacity_available() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.wait_timeout = Duration::from_millis(30);
        policy.max_pending = 8;
        policy.max_pending_per_principal = 8;

        {
            let mut state = RPC_ADMISSION_QUEUE
                .state
                .lock()
                .expect("rpc admission queue mutex should not be poisoned");
            state.admitted_total = 1;
            let existing_pending = state.allocate_request_id();
            state.enqueue_request("principal-a", existing_pending);
        }

        let err = try_acquire_rpc_queue_permit("principal-b", "test://rpc", "eth_call", 2, &policy)
            .expect_err("new arrival should not bypass existing pending waiter");
        let err_text = format!("{err}");
        assert!(
            err_text.contains("queue timeout"),
            "new arrival should remain queued behind existing pending waiter: {err_text}"
        );

        let state = RPC_ADMISSION_QUEUE
            .state
            .lock()
            .expect("rpc admission queue mutex should not be poisoned");
        assert_eq!(
            state.admitted_total, 2,
            "existing pending should be promoted into available capacity before newcomer"
        );
        assert_eq!(
            state.pending_for_principal("principal-b"),
            0,
            "timed-out newcomer should be removed from pending queue"
        );
    }

    #[test]
    fn queue_rejects_immediately_when_zero_pending_capacity() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.max_pending = 0;
        policy.max_pending_per_principal = 0;

        let err = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 0, &policy)
            .expect_err("zero pending capacity should reject immediately");
        let text = format!("{err}");
        assert!(
            text.contains("global pending limit"),
            "unexpected rejection: {text}"
        );
    }

    #[test]
    fn queue_times_out_when_global_limit_is_zero() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        let mut policy = test_queue_policy();
        policy.max_pending = 8;
        policy.max_pending_per_principal = 8;
        policy.wait_timeout = Duration::from_millis(25);

        let err = try_acquire_rpc_queue_permit("principal-a", "test://rpc", "eth_call", 0, &policy)
            .expect_err("without global capacity queued request should time out");
        let text = format!("{err}");
        assert!(
            text.contains("queue timeout"),
            "timeout should be explicit when global limit is zero: {text}"
        );
    }

    #[test]
    fn admission_promote_with_zero_global_limit_grants_nothing() {
        let mut state = AdmissionState::default();
        let policy = test_queue_policy();
        state.enqueue_request("principal-a", 1);
        state.enqueue_request("principal-b", 2);

        let granted = state.promote_admissions(&policy, 0);
        assert_eq!(granted, 0, "zero global limit should block promotions");
        assert_eq!(state.admitted_total, 0);
        assert_eq!(state.pending_total, 2);
    }

    #[test]
    fn queue_admission_keeps_existing_inflight_caps_enforced() {
        let _guard = QUEUE_TEST_LOCK
            .lock()
            .expect("queue test lock should not be poisoned");
        reset_rpc_admission_queue_state();
        RPC_INFLIGHT_BY_ENDPOINT
            .lock()
            .expect("endpoint in-flight mutex should not be poisoned")
            .clear();
        RPC_INFLIGHT_BY_PRINCIPAL
            .lock()
            .expect("principal in-flight mutex should not be poisoned")
            .clear();
        RPC_INFLIGHT_BY_SCOPE
            .lock()
            .expect("scope in-flight mutex should not be poisoned")
            .clear();
        RPC_INFLIGHT_BY_METHOD
            .lock()
            .expect("method in-flight mutex should not be poisoned")
            .clear();

        let policy = test_queue_policy();
        let permit = try_acquire_rpc_queue_permit("principal-cap", "test://rpc", "eth_call", 8, &policy)
            .expect("permit should be granted")
            .expect("queue should return permit");

        let endpoint_key = format!("endpoint-cap-{}", std::process::id());
        let principal_key = format!("principal-cap-{}", std::process::id());
        let scope_key = format!("scope-cap-{}", std::process::id());
        let method_key = format!("method-cap-{}", std::process::id());

        let endpoint_guard = try_acquire_rpc_endpoint_inflight_slot(&endpoint_key, 1)
            .expect("endpoint slot should be acquirable");
        assert!(
            try_acquire_rpc_endpoint_inflight_slot(&endpoint_key, 1).is_none(),
            "endpoint cap should remain fail-fast"
        );
        drop(endpoint_guard);

        let principal_guard = try_acquire_rpc_principal_inflight_slot(&principal_key, 1)
            .expect("principal slot should be acquirable");
        assert!(
            try_acquire_rpc_principal_inflight_slot(&principal_key, 1).is_none(),
            "principal cap should remain fail-fast"
        );
        drop(principal_guard);

        let scope_guard =
            try_acquire_rpc_scope_inflight_slot(&scope_key, 1).expect("scope slot should be acquirable");
        assert!(
            try_acquire_rpc_scope_inflight_slot(&scope_key, 1).is_none(),
            "scope cap should remain fail-fast"
        );
        drop(scope_guard);

        let method_guard =
            try_acquire_rpc_method_inflight_slot(&method_key, 1).expect("method slot should be acquirable");
        assert!(
            try_acquire_rpc_method_inflight_slot(&method_key, 1).is_none(),
            "method cap should remain fail-fast"
        );
        drop(method_guard);

        drop(permit);
    }

    #[test]
    fn remove_pending_request_cleans_idle_principal_state() {
        let mut state = AdmissionState::default();
        state.enqueue_request("principal-cleanup", 42);
        assert_eq!(state.pending_total, 1);
        assert_eq!(state.pending_for_principal("principal-cleanup"), 1);

        assert!(
            state.remove_pending_request("principal-cleanup", 42),
            "existing pending request should be removable"
        );
        assert_eq!(state.pending_total, 0);
        assert_eq!(state.pending_for_principal("principal-cleanup"), 0);
        assert!(
            !state.pending_by_principal.contains_key("principal-cleanup"),
            "principal queue should be fully cleaned when idle"
        );
        assert!(
            !state.pending_count_by_principal.contains_key("principal-cleanup"),
            "principal pending counter should be removed when idle"
        );
        assert!(
            !state
                .principal_ring
                .iter()
                .any(|principal| principal == "principal-cleanup"),
            "principal should not remain in ring after cleanup"
        );
    }

    #[test]
    fn take_admitted_returns_false_for_unknown_request() {
        let mut state = AdmissionState::default();
        assert!(
            !state.take_admitted(99),
            "unknown admitted request should return false"
        );
    }

    #[test]
    fn release_admission_saturates_at_zero() {
        let mut state = AdmissionState::default();
        state.release_admission();
        assert_eq!(
            state.admitted_total, 0,
            "release should not underflow admitted count"
        );
    }

    #[test]
    fn remove_pending_request_returns_false_for_unknown_request() {
        let mut state = AdmissionState::default();
        state.enqueue_request("principal-a", 1);
        state.enqueue_request("principal-b", 2);

        assert!(
            !state.remove_pending_request("principal-a", 999),
            "unknown request ID for existing principal should return false"
        );
        assert!(
            !state.remove_pending_request("principal-missing", 2),
            "unknown principal should return false"
        );
        assert_eq!(state.pending_total, 2, "state should remain unchanged");
        assert_eq!(state.pending_for_principal("principal-a"), 1);
        assert_eq!(state.pending_for_principal("principal-b"), 1);
    }

    #[test]
    fn enqueue_request_keeps_single_ring_entry_per_principal() {
        let mut state = AdmissionState::default();
        state.enqueue_request("principal-a", 1);
        state.enqueue_request("principal-a", 2);
        state.enqueue_request("principal-a", 3);

        let matches = state
            .principal_ring
            .iter()
            .filter(|principal| principal.as_str() == "principal-a")
            .count();
        assert_eq!(matches, 1, "principal should appear only once in ring");
        assert_eq!(state.pending_for_principal("principal-a"), 3);
    }

    #[test]
    fn remove_pending_request_preserves_remaining_fifo_order() {
        let mut state = AdmissionState::default();
        let policy = test_queue_policy();
        state.enqueue_request("principal-a", 10);
        state.enqueue_request("principal-a", 11);
        state.enqueue_request("principal-a", 12);

        assert!(
            state.remove_pending_request("principal-a", 11),
            "middle request should be removable"
        );

        let first = state
            .admit_next_request(&policy)
            .expect("first remaining request should be admissible");
        assert_eq!(first, 10, "oldest remaining request should admit first");
        assert!(state.take_admitted(first));
        state.release_admission();

        let second = state
            .admit_next_request(&policy)
            .expect("second remaining request should be admissible");
        assert_eq!(second, 12, "newest remaining request should admit last");
    }

    #[test]
    fn drr_scheduler_reuses_remaining_deficit_before_round_robin_switch() {
        let mut state = AdmissionState::default();
        let mut policy = test_queue_policy();
        policy.drr_quantum = 2;

        state.enqueue_request("principal-a", 1);
        state.enqueue_request("principal-a", 2);
        state.enqueue_request("principal-b", 3);

        let first = state
            .admit_next_request(&policy)
            .expect("first admission should succeed");
        assert_eq!(first, 1);
        assert_eq!(
            state.principal_ring.front().map(String::as_str),
            Some("principal-a"),
            "principal with remaining deficit should stay at front"
        );

        let second = state
            .admit_next_request(&policy)
            .expect("second admission should succeed");
        assert_eq!(second, 2);
        assert_eq!(
            state.principal_ring.front().map(String::as_str),
            Some("principal-b"),
            "principal should rotate after deficit is consumed or queue empties"
        );
    }

    #[test]
    fn admit_next_request_cleans_principal_state_when_queue_drains() {
        let mut state = AdmissionState::default();
        let policy = test_queue_policy();
        state.enqueue_request("principal-a", 10);
        state.deficit_by_principal.insert("principal-a".to_owned(), 7);

        let request_id = state
            .admit_next_request(&policy)
            .expect("single queued request should admit");
        assert_eq!(request_id, 10);
        assert_eq!(state.pending_total, 0);
        assert_eq!(state.pending_for_principal("principal-a"), 0);
        assert!(
            !state.pending_by_principal.contains_key("principal-a"),
            "empty principal queue should be cleaned"
        );
        assert!(
            !state.deficit_by_principal.contains_key("principal-a"),
            "deficit state should be cleaned when principal becomes idle"
        );
    }

    #[test]
    fn remove_pending_request_returns_false_for_already_admitted_request() {
        let mut state = AdmissionState::default();
        let policy = test_queue_policy();
        state.enqueue_request("principal-a", 42);
        let admitted = state
            .admit_next_request(&policy)
            .expect("request should admit");
        assert_eq!(admitted, 42);
        assert!(
            !state.remove_pending_request("principal-a", 42),
            "already admitted request should not appear in pending queue"
        );
        assert_eq!(state.pending_total, 0);
        assert_eq!(state.admitted_total, 1);
    }

    #[test]
    fn clear_principal_if_idle_cleans_deficit_and_ring() {
        let mut state = AdmissionState::default();
        state.principal_ring.push_back("principal-a".to_owned());
        state.pending_count_by_principal.insert("principal-a".to_owned(), 0);
        state.deficit_by_principal.insert("principal-a".to_owned(), 7);
        state.pending_by_principal
            .insert("principal-a".to_owned(), VecDeque::new());

        state.clear_principal_if_idle("principal-a");

        assert!(
            !state
                .principal_ring
                .iter()
                .any(|principal| principal == "principal-a"),
            "idle principal should be removed from round-robin ring"
        );
        assert!(
            !state.deficit_by_principal.contains_key("principal-a"),
            "idle principal deficit should be cleaned"
        );
        assert!(
            !state.pending_count_by_principal.contains_key("principal-a"),
            "idle principal pending count should be cleaned"
        );
        assert!(
            !state.pending_by_principal.contains_key("principal-a"),
            "idle principal queue should be cleaned"
        );
    }

    #[test]
    fn next_admissible_principal_skips_empty_principals_in_ring() {
        let mut state = AdmissionState::default();
        let policy = test_queue_policy();

        state.principal_ring.push_back("stale-principal".to_owned());
        state.pending_count_by_principal.insert("stale-principal".to_owned(), 0);
        state
            .pending_by_principal
            .insert("stale-principal".to_owned(), VecDeque::new());
        state.enqueue_request("principal-live", 10);

        let principal = state
            .next_admissible_principal(&policy)
            .expect("live principal should be selected");
        assert_eq!(principal, "principal-live");
        assert!(
            !state.deficit_by_principal.contains_key("stale-principal"),
            "stale principal state should be cleaned during scheduling"
        );
    }

    #[test]
    fn next_admissible_principal_returns_none_for_empty_ring() {
        let mut state = AdmissionState::default();
        let policy = test_queue_policy();
        assert!(
            state.next_admissible_principal(&policy).is_none(),
            "empty ring should have no admissible principal"
        );
    }

    #[test]
    fn drr_no_starvation_for_lower_weight_principal() {
        let mut state = AdmissionState::default();
        let mut policy = test_queue_policy();
        policy.principal_weights.insert("principal-heavy".to_owned(), 8);
        policy.principal_weights.insert("principal-light".to_owned(), 1);
        let mut owner_by_request_id = HashMap::<RequestId, String>::new();

        for idx in 0..500u64 {
            let heavy_id = idx + 1;
            let light_id = idx + 1_000;
            state.enqueue_request("principal-heavy", heavy_id);
            state.enqueue_request("principal-light", light_id);
            owner_by_request_id.insert(heavy_id, "principal-heavy".to_owned());
            owner_by_request_id.insert(light_id, "principal-light".to_owned());
        }

        let mut light_admissions = 0usize;
        for _ in 0..180usize {
            let request_id = state
                .admit_next_request(&policy)
                .expect("admission should be available");
            if owner_by_request_id
                .get(&request_id)
                .map(|principal| principal == "principal-light")
                .unwrap_or(false)
            {
                light_admissions = light_admissions.saturating_add(1);
            }
            assert!(
                state.take_admitted(request_id),
                "admitted request should be visible to waiter"
            );
            state.release_admission();
        }

        assert!(
            light_admissions > 0,
            "lower-weight principal should still receive progress"
        );
        assert!(
            light_admissions >= 15,
            "lower-weight principal should continue to make measurable progress, got {light_admissions}"
        );
    }

    #[test]
    fn parse_timeout_ms_uses_default_for_missing_value() {
        let timeout = parse_timeout_ms(None, 1234);
        assert_eq!(timeout, Duration::from_millis(1234));
    }

    #[test]
    fn parse_timeout_ms_uses_default_for_invalid_or_zero_value() {
        let invalid = parse_timeout_ms(Some("oops".to_owned()), 1234);
        assert_eq!(invalid, Duration::from_millis(1234));

        let zero = parse_timeout_ms(Some("0".to_owned()), 1234);
        assert_eq!(zero, Duration::from_millis(1234));
    }

    #[test]
    fn parse_timeout_ms_parses_positive_value() {
        let timeout = parse_timeout_ms(Some("2500".to_owned()), 1234);
        assert_eq!(timeout, Duration::from_millis(2500));
    }

    #[test]
    fn parse_retry_count_clamps_and_uses_default() {
        assert_eq!(parse_retry_count(None, 3), 3);
        assert_eq!(parse_retry_count(Some("oops".to_owned()), 3), 3);
        assert_eq!(parse_retry_count(Some("1".to_owned()), 3), 1);
        assert_eq!(parse_retry_count(Some("99".to_owned()), 3), 10);
    }

    #[test]
    fn parse_backoff_ms_allows_zero_and_uses_default_on_invalid() {
        assert_eq!(parse_backoff_ms(None, 250), Duration::from_millis(250));
        assert_eq!(
            parse_backoff_ms(Some("oops".to_owned()), 250),
            Duration::from_millis(250)
        );
        assert_eq!(
            parse_backoff_ms(Some("0".to_owned()), 250),
            Duration::from_millis(0)
        );
    }

    #[test]
    fn parse_failure_threshold_clamps_and_uses_default() {
        assert_eq!(parse_failure_threshold(None, 5), 5);
        assert_eq!(parse_failure_threshold(Some("oops".to_owned()), 5), 5);
        assert_eq!(parse_failure_threshold(Some("7".to_owned()), 5), 7);
        assert_eq!(parse_failure_threshold(Some("999".to_owned()), 5), 100);
    }

    #[test]
    fn parse_inflight_limit_uses_default_for_invalid_or_zero() {
        assert_eq!(parse_inflight_limit(None, 32), 32);
        assert_eq!(parse_inflight_limit(Some("oops".to_owned()), 32), 32);
        assert_eq!(parse_inflight_limit(Some("0".to_owned()), 32), 32);
    }

    #[test]
    fn parse_inflight_limit_clamps_to_maximum() {
        assert_eq!(parse_inflight_limit(Some("7".to_owned()), 32), 7);
        assert_eq!(
            parse_inflight_limit(Some("99999".to_owned()), 32),
            MAX_RPC_MAX_INFLIGHT
        );
    }

    #[test]
    fn normalize_per_endpoint_inflight_limit_caps_to_global_limit() {
        assert_eq!(normalize_per_endpoint_inflight_limit(32, 16), 16);
        assert_eq!(normalize_per_endpoint_inflight_limit(8, 16), 8);
        assert_eq!(normalize_per_endpoint_inflight_limit(0, 16), 0);
    }

    #[test]
    fn normalize_per_principal_inflight_limit_caps_to_global_limit() {
        assert_eq!(normalize_per_principal_inflight_limit(32, 12), 12);
        assert_eq!(normalize_per_principal_inflight_limit(8, 12), 8);
        assert_eq!(normalize_per_principal_inflight_limit(0, 12), 0);
    }

    #[test]
    fn normalize_per_scope_inflight_limit_caps_to_global_limit() {
        assert_eq!(normalize_per_scope_inflight_limit(32, 12), 12);
        assert_eq!(normalize_per_scope_inflight_limit(8, 12), 8);
        assert_eq!(normalize_per_scope_inflight_limit(0, 12), 0);
    }

    #[test]
    fn normalize_per_method_inflight_limit_caps_to_endpoint_and_global() {
        assert_eq!(normalize_per_method_inflight_limit(32, 16, 8), 8);
        assert_eq!(normalize_per_method_inflight_limit(32, 16, 24), 16);
        assert_eq!(normalize_per_method_inflight_limit(8, 16, 24), 8);
        assert_eq!(normalize_per_method_inflight_limit(8, 0, 4), 0);
        assert_eq!(normalize_per_method_inflight_limit(0, 8, 4), 0);
    }

    #[test]
    fn method_is_retry_safe_excludes_submit_and_send_methods() {
        assert!(method_is_retry_safe("eth_call"));
        assert!(method_is_retry_safe("state_getStorage"));
        assert!(!method_is_retry_safe("author_submitExtrinsic"));
        assert!(!method_is_retry_safe("eth_sendRawTransaction"));
        assert!(!method_is_retry_safe("sendTransaction"));
        assert!(!method_is_retry_safe("sendBoc"));
    }

    #[test]
    fn is_retryable_http_status_matches_expected_set() {
        assert!(is_retryable_http_status(408));
        assert!(is_retryable_http_status(429));
        assert!(is_retryable_http_status(503));
        assert!(!is_retryable_http_status(400));
        assert!(!is_retryable_http_status(404));
    }

    #[test]
    fn retry_delay_scales_linearly_by_attempt() {
        let base = Duration::from_millis(200);
        assert_eq!(retry_delay(base, 0), Duration::from_millis(200));
        assert_eq!(retry_delay(base, 1), Duration::from_millis(400));
        assert_eq!(retry_delay(base, 2), Duration::from_millis(600));
    }

    #[test]
    fn retry_delay_saturates_on_overflow() {
        let base = Duration::from_millis(u64::MAX);
        assert_eq!(
            retry_delay(base, u32::MAX),
            Duration::from_millis(u64::MAX),
            "overflowing backoff multiplication should saturate"
        );
    }

    #[test]
    fn rpc_inflight_guard_enforces_limit_and_releases_on_drop() {
        RPC_INFLIGHT.store(0, Ordering::Release);

        let guard =
            try_acquire_rpc_inflight_slot(1).expect("first acquisition at limit should succeed");
        assert!(
            try_acquire_rpc_inflight_slot(1).is_none(),
            "second acquisition should fail when at limit"
        );
        drop(guard);
        let guard2 = try_acquire_rpc_inflight_slot(1)
            .expect("acquisition should succeed after previous guard drops");
        drop(guard2);

        RPC_INFLIGHT.store(0, Ordering::Release);
    }

    #[test]
    fn rpc_inflight_slot_rejects_zero_limit() {
        RPC_INFLIGHT.store(0, Ordering::Release);
        assert!(
            try_acquire_rpc_inflight_slot(0).is_none(),
            "zero global in-flight limit should reject immediately"
        );
        assert_eq!(
            RPC_INFLIGHT.load(Ordering::Acquire),
            0,
            "counter should remain unchanged on zero limit"
        );
    }

    #[test]
    fn rpc_endpoint_inflight_guard_enforces_limit_and_releases_on_drop() {
        let key_a = format!("test://endpoint-a-{}", std::process::id());
        let key_b = format!("test://endpoint-b-{}", std::process::id());
        RPC_INFLIGHT_BY_ENDPOINT
            .lock()
            .expect("endpoint in-flight mutex should not be poisoned")
            .clear();

        let guard_a = try_acquire_rpc_endpoint_inflight_slot(&key_a, 1)
            .expect("first acquisition for endpoint A should succeed");
        assert!(
            try_acquire_rpc_endpoint_inflight_slot(&key_a, 1).is_none(),
            "second acquisition for endpoint A should fail at endpoint limit"
        );

        let guard_b = try_acquire_rpc_endpoint_inflight_slot(&key_b, 1)
            .expect("endpoint B should be independently acquirable");

        drop(guard_a);
        let guard_a2 = try_acquire_rpc_endpoint_inflight_slot(&key_a, 1)
            .expect("endpoint A should be acquirable after guard drop");

        drop(guard_a2);
        drop(guard_b);

        let by_endpoint = RPC_INFLIGHT_BY_ENDPOINT
            .lock()
            .expect("endpoint in-flight mutex should not be poisoned");
        assert!(
            !by_endpoint.contains_key(&key_a) && !by_endpoint.contains_key(&key_b),
            "endpoint counters should be cleaned up at zero"
        );
    }

    #[test]
    fn rpc_endpoint_inflight_slot_rejects_zero_limit() {
        let key = format!("test://endpoint-zero-{}", std::process::id());
        RPC_INFLIGHT_BY_ENDPOINT
            .lock()
            .expect("endpoint in-flight mutex should not be poisoned")
            .clear();
        assert!(
            try_acquire_rpc_endpoint_inflight_slot(&key, 0).is_none(),
            "zero endpoint limit should reject immediately"
        );
        let by_endpoint = RPC_INFLIGHT_BY_ENDPOINT
            .lock()
            .expect("endpoint in-flight mutex should not be poisoned");
        assert!(
            !by_endpoint.contains_key(&key),
            "zero-limit acquire should not mutate endpoint counters"
        );
    }

    #[test]
    fn rpc_principal_inflight_guard_enforces_limit_and_releases_on_drop() {
        let key_a = format!("principal_a_{}", std::process::id());
        let key_b = format!("principal_b_{}", std::process::id());
        RPC_INFLIGHT_BY_PRINCIPAL
            .lock()
            .expect("principal in-flight mutex should not be poisoned")
            .clear();

        let guard_a = try_acquire_rpc_principal_inflight_slot(&key_a, 1)
            .expect("first acquisition for principal key A should succeed");
        assert!(
            try_acquire_rpc_principal_inflight_slot(&key_a, 1).is_none(),
            "second acquisition for principal key A should fail at principal limit"
        );

        let guard_b = try_acquire_rpc_principal_inflight_slot(&key_b, 1)
            .expect("principal key B should be independently acquirable");

        drop(guard_a);
        let guard_a2 = try_acquire_rpc_principal_inflight_slot(&key_a, 1)
            .expect("principal key A should be acquirable after guard drop");

        drop(guard_a2);
        drop(guard_b);

        let by_principal = RPC_INFLIGHT_BY_PRINCIPAL
            .lock()
            .expect("principal in-flight mutex should not be poisoned");
        assert!(
            !by_principal.contains_key(&key_a) && !by_principal.contains_key(&key_b),
            "principal counters should be cleaned up at zero"
        );
    }

    #[test]
    fn rpc_principal_inflight_slot_rejects_zero_limit() {
        let key = format!("principal_zero_{}", std::process::id());
        RPC_INFLIGHT_BY_PRINCIPAL
            .lock()
            .expect("principal in-flight mutex should not be poisoned")
            .clear();
        assert!(
            try_acquire_rpc_principal_inflight_slot(&key, 0).is_none(),
            "zero principal limit should reject immediately"
        );
        let by_principal = RPC_INFLIGHT_BY_PRINCIPAL
            .lock()
            .expect("principal in-flight mutex should not be poisoned");
        assert!(
            !by_principal.contains_key(&key),
            "zero-limit acquire should not mutate principal counters"
        );
    }

    #[test]
    fn rpc_scope_inflight_guard_enforces_limit_and_releases_on_drop() {
        let key_a = format!("sccp_health_{}", std::process::id());
        let key_b = format!("sccp_get_token_state_{}", std::process::id());
        RPC_INFLIGHT_BY_SCOPE
            .lock()
            .expect("scope in-flight mutex should not be poisoned")
            .clear();

        let guard_a = try_acquire_rpc_scope_inflight_slot(&key_a, 1)
            .expect("first acquisition for scope key A should succeed");
        assert!(
            try_acquire_rpc_scope_inflight_slot(&key_a, 1).is_none(),
            "second acquisition for scope key A should fail at scope limit"
        );

        let guard_b = try_acquire_rpc_scope_inflight_slot(&key_b, 1)
            .expect("scope key B should be independently acquirable");

        drop(guard_a);
        let guard_a2 = try_acquire_rpc_scope_inflight_slot(&key_a, 1)
            .expect("scope key A should be acquirable after guard drop");

        drop(guard_a2);
        drop(guard_b);

        let by_scope = RPC_INFLIGHT_BY_SCOPE
            .lock()
            .expect("scope in-flight mutex should not be poisoned");
        assert!(
            !by_scope.contains_key(&key_a) && !by_scope.contains_key(&key_b),
            "scope counters should be cleaned up at zero"
        );
    }

    #[test]
    fn rpc_scope_inflight_slot_rejects_zero_limit() {
        let key = format!("scope_zero_{}", std::process::id());
        RPC_INFLIGHT_BY_SCOPE
            .lock()
            .expect("scope in-flight mutex should not be poisoned")
            .clear();
        assert!(
            try_acquire_rpc_scope_inflight_slot(&key, 0).is_none(),
            "zero scope limit should reject immediately"
        );
        let by_scope = RPC_INFLIGHT_BY_SCOPE
            .lock()
            .expect("scope in-flight mutex should not be poisoned");
        assert!(
            !by_scope.contains_key(&key),
            "zero-limit acquire should not mutate scope counters"
        );
    }

    #[test]
    fn rpc_method_inflight_guard_enforces_limit_and_releases_on_drop() {
        let key_a = format!("test://endpoint-a-{}::system_chain", std::process::id());
        let key_b = format!("test://endpoint-a-{}::chain_getHeader", std::process::id());
        RPC_INFLIGHT_BY_METHOD
            .lock()
            .expect("method in-flight mutex should not be poisoned")
            .clear();

        let guard_a = try_acquire_rpc_method_inflight_slot(&key_a, 1)
            .expect("first acquisition for method key A should succeed");
        assert!(
            try_acquire_rpc_method_inflight_slot(&key_a, 1).is_none(),
            "second acquisition for method key A should fail at method limit"
        );

        let guard_b = try_acquire_rpc_method_inflight_slot(&key_b, 1)
            .expect("method key B should be independently acquirable");

        drop(guard_a);
        let guard_a2 = try_acquire_rpc_method_inflight_slot(&key_a, 1)
            .expect("method key A should be acquirable after guard drop");

        drop(guard_a2);
        drop(guard_b);

        let by_method = RPC_INFLIGHT_BY_METHOD
            .lock()
            .expect("method in-flight mutex should not be poisoned");
        assert!(
            !by_method.contains_key(&key_a) && !by_method.contains_key(&key_b),
            "method counters should be cleaned up at zero"
        );
    }

    #[test]
    fn rpc_method_inflight_slot_rejects_zero_limit() {
        let key = format!("test://endpoint-zero-{}::system_chain", std::process::id());
        RPC_INFLIGHT_BY_METHOD
            .lock()
            .expect("method in-flight mutex should not be poisoned")
            .clear();
        assert!(
            try_acquire_rpc_method_inflight_slot(&key, 0).is_none(),
            "zero method limit should reject immediately"
        );
        let by_method = RPC_INFLIGHT_BY_METHOD
            .lock()
            .expect("method in-flight mutex should not be poisoned");
        assert!(
            !by_method.contains_key(&key),
            "zero-limit acquire should not mutate method counters"
        );
    }

    #[test]
    fn with_rpc_fairness_scope_sets_and_restores_scope() {
        RPC_FAIRNESS_SCOPE.with(|scope| {
            scope.replace(None);
        });
        assert_eq!(current_rpc_fairness_scope(), None);

        with_rpc_fairness_scope("sccp_health", || {
            assert_eq!(current_rpc_fairness_scope().as_deref(), Some("sccp_health"));
            with_rpc_fairness_scope("sccp_get_token_state", || {
                assert_eq!(
                    current_rpc_fairness_scope().as_deref(),
                    Some("sccp_get_token_state")
                );
            });
            assert_eq!(current_rpc_fairness_scope().as_deref(), Some("sccp_health"));
        });

        assert_eq!(current_rpc_fairness_scope(), None);
    }

    #[test]
    fn with_rpc_fairness_principal_sets_and_restores_principal() {
        RPC_FAIRNESS_PRINCIPAL.with(|principal| {
            principal.replace(None);
        });
        assert_eq!(current_rpc_fairness_principal(), None);

        with_rpc_fairness_principal("requester_a", || {
            assert_eq!(
                current_rpc_fairness_principal().as_deref(),
                Some("requester_a")
            );
            with_rpc_fairness_principal("requester_b", || {
                assert_eq!(
                    current_rpc_fairness_principal().as_deref(),
                    Some("requester_b")
                );
            });
            assert_eq!(
                current_rpc_fairness_principal().as_deref(),
                Some("requester_a")
            );
        });

        assert_eq!(current_rpc_fairness_principal(), None);
    }

    #[test]
    fn circuit_breaker_opens_after_threshold() {
        let key = format!("test://{}", std::process::id());
        let policy = CircuitBreakerPolicy {
            failure_threshold: 2,
            cooldown: Duration::from_secs(30),
        };
        record_circuit_success(&key);

        assert!(
            circuit_open_remaining(&key, policy).is_none(),
            "circuit should start closed"
        );
        record_circuit_failure(&key, policy);
        assert!(
            circuit_open_remaining(&key, policy).is_none(),
            "first failure should not open breaker"
        );
        record_circuit_failure(&key, policy);
        assert!(
            circuit_open_remaining(&key, policy).is_some(),
            "breaker should open after threshold"
        );
        record_circuit_success(&key);
    }

    #[test]
    fn circuit_breaker_disabled_when_threshold_is_zero() {
        let key = format!("test://disabled-{}", std::process::id());
        let policy = CircuitBreakerPolicy {
            failure_threshold: 0,
            cooldown: Duration::from_secs(30),
        };

        record_circuit_failure(&key, policy);
        assert!(
            circuit_open_remaining(&key, policy).is_none(),
            "threshold=0 should disable breaker"
        );
        record_circuit_success(&key);
    }

    #[test]
    fn circuit_breaker_success_resets_failure_state() {
        let key = format!("test://reset-{}", std::process::id());
        let policy = CircuitBreakerPolicy {
            failure_threshold: 2,
            cooldown: Duration::from_secs(30),
        };
        record_circuit_success(&key);

        record_circuit_failure(&key, policy);
        record_circuit_success(&key);
        assert!(
            circuit_open_remaining(&key, policy).is_none(),
            "success should clear failure state"
        );
    }

    #[test]
    fn circuit_open_remaining_clears_expired_open_state() {
        let key = format!("test://expired-{}", std::process::id());
        {
            let mut states = circuit_states()
                .lock()
                .expect("circuit state mutex should not be poisoned");
            states.insert(
                key.clone(),
                CircuitState {
                    consecutive_failures: 9,
                    open_until: Some(Instant::now() - Duration::from_millis(1)),
                },
            );
        }

        let policy = CircuitBreakerPolicy {
            failure_threshold: 1,
            cooldown: Duration::from_secs(1),
        };
        assert!(
            circuit_open_remaining(&key, policy).is_none(),
            "expired open state should be treated as closed"
        );

        let states = circuit_states()
            .lock()
            .expect("circuit state mutex should not be poisoned");
        let state = states
            .get(&key)
            .expect("expired state entry should remain present");
        assert_eq!(
            state.consecutive_failures, 0,
            "expired state should reset consecutive failures"
        );
        assert!(
            state.open_until.is_none(),
            "expired state should clear open_until"
        );
    }
}
