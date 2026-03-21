# SCCP EVM Inbound Anchor Mode

This document is retained only as a tombstone for a removed SCCP fallback path.

- `EvmAnchor` is no longer a supported inbound finality mode.
- `set_evm_anchor_mode_enabled` and `set_evm_inbound_anchor` were removed from the active SCCP governance surface.
- Reserved legacy enum slots remain only for SCALE compatibility and fail closed.

Use the current proof-backed paths documented in [FINALITY.md](/Users/mtakemiya/dev/sora2-network/docs/sccp/FINALITY.md) instead.
