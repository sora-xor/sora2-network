# SORA Nexus modification (2026-10-24): Disable the unused light-client transport.
# The published Python smoldot-light 0.1.0 extension bundles vulnerable Rust
# dependencies that pip-audit cannot inspect. The governance CLI uses URL RPC.

from .base import TransportBase
from ..exceptions import ConfigurationError


class SmoldotTransport(TransportBase):
    """Reject light-client mode in this HTTP/WebSocket governance client."""

    def __init__(self, chainspec, relay_chainspecs=None, relay_chain_ids=None, debug_fn=None):
        raise ConfigurationError(
            "Light-client transport is not supported by the runtime-upgrade client; "
            "use an HTTP or WebSocket RPC URL"
        )

    def rpc_request(self, payload, result_handler=None):
        raise ConfigurationError("Light-client transport is not supported by the runtime-upgrade client")
