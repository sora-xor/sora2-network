# SORA Nexus native DOMException migration

Published consumer: fetch-blob 3.2.0. Archive integrity: `sha512-7yAQpD2UMJzLi1Dqv7qFYnPbaPx7ZfFK6PiIxQ4PfkGPyNyl2Ugx+a/umUonmKqjhM4DnfbMvdX6otXq83soQQ==`.
Archive SHA-256: `4abf0d58a4977fce2240e08c280a2bc59f5363e9553a4f236cea6d74cce40c52`.

Keep the original fetch-blob implementation and package version, including
Blob/File streaming, content types, slicing and filesystem consistency checks.
Replace its one node-domexception import in from.js with Node's native
`globalThis.DOMException`, and remove the deprecated shim dependency. On the
supported Node 24+ runtime the old shim already returned this same constructor.
No fetch transport, proxy-agent, or HTTP semantics are changed. The manifest
requires Node 24+ consistently with the operational tooling.

The original license is retained. Tests exercise synchronous/asynchronous file
Blobs and Files, slicing/streaming and the native DOMException raised when a
file changes after the Blob was created.
