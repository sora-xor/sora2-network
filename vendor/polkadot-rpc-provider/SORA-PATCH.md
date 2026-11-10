# SORA Nexus dependency patch

Exact published `@polkadot/rpc-provider` 16.5.6 archive; registry integrity and SHA-256 are recorded in UPSTREAM.json.

Removed the unused optional `@substrate/connect` 0.8.11 dependency, which installed the vulnerable prebuilt smoldot 2.0.26 Wasm bundle. The provider never imported that package at runtime: ScProvider already requires callers to inject its client interface. All HTTP, WebSocket, and ScProvider JavaScript is unchanged. Standalone structural declarations describe that injected interface without requiring the removed optional package. The supported operational tools use HTTP/WebSocket; this distribution does not bundle or provide a light client.
