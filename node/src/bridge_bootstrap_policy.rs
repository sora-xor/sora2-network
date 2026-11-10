//! Local bootstrap policy for SORA Nexus recovery bridge peers.
//!
//! Preparing local configuration never opens the recovery or on-chain halt gates.

#[derive(Clone, Copy)]
pub(crate) struct BridgeWorkerOptions {
    pub recovery_bootstrap: bool,
    pub resume_after_halt: Option<u32>,
}

pub(crate) fn enabled(
    recovery: bool,
    offline: bool,
    offchain_workers_enabled: bool,
    recovery_opt_in: bool,
) -> bool {
    !offline && (!recovery || (offchain_workers_enabled && recovery_opt_in))
}

#[cfg(test)]
mod tests {
    use super::enabled;

    #[test]
    fn recovery_authority_default_does_not_rearm_retained_keys() {
        assert!(!enabled(true, false, true, false));
    }

    #[test]
    fn recovery_explicit_always_prepares_local_configuration() {
        assert!(enabled(true, false, true, true));
    }

    #[test]
    fn disabled_workers_cannot_opt_in_to_recovery_bootstrap() {
        assert!(!enabled(true, false, false, true));
        assert!(!enabled(true, false, false, false));
    }

    #[test]
    fn offline_commands_never_bootstrap() {
        for recovery in [false, true] {
            for workers_enabled in [false, true] {
                for opt_in in [false, true] {
                    assert!(!enabled(recovery, true, workers_enabled, opt_in));
                }
            }
        }
    }

    #[test]
    fn ordinary_online_bootstrap_keeps_existing_behavior() {
        for workers_enabled in [false, true] {
            for opt_in in [false, true] {
                assert!(enabled(false, false, workers_enabled, opt_in));
            }
        }
    }
}
