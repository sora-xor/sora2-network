# SORA Nexus 4.8.15 runtime upgrade

Runtime spec **142**, transaction version **134**. The publication location for
release binaries, Wasm, checksums and validation evidence is the
[4.8.15 release](https://github.com/sora-xor/sora2-network/releases/tag/4.8.15).

The compressed Wasm's BLAKE2-256 code hash is
`0xd601b781566850086b9341552bcde75b43902665a01fc94897da2adf3cac9a26`; its SHA-256 is `6c1d8a0129f144bea4d3498d1fc2c0baf86ab937f401d1103e6eabfc28a90a27`.
The node commits this exact code hash as its worker's accepted mainnet runtime.

## Halt behavior

Guardian halts are OCW stop signals. Guardian signals and the historical recovery
halt marker no longer reject ordinary bridge operations at consensus level after
recovery is coherently complete. The historical raw recovery halt marker is not
a replacement for a new guardian halt.
The node durably records each observed guardian halt, including a local generation,
and requires an explicit acknowledgement of that exact observation before its
worker resumes. Reorganization or daemon restart cannot release that local stop.
A new halt requires a new acknowledgement.

Completed recovery requires its own local, digest-bound acknowledgement. Neither
acknowledgement option enables OCW, installs keys, changes chain storage, or clears
another acknowledgement. Unfinished or malformed recovery state still blocks
ordinary bridge dispatch. Permanent quarantine, recorded attacker handling and the
seven-request node-local block remain in force. Unrecognized retained retry calls
are retained without signing.

See the [operator procedure](../misc/bridge-halt-web/README.md) for exact local
acknowledgement and scan-cursor behavior. The previous numeric halt option cannot
release a pending halt observed by this node.

## Adoption

Upgrade bridge nodes before adopting spec 142. The node checks the exact runtime
code and genesis before permitting worker key access or local submission; it will
not sign under an unreviewed runtime. Continue to pass `--offchain-worker never`
on nodes that must remain disabled.

The currently deployed runtime must adopt this Wasm through its existing upgrade
authorization. Once adopted, restarting a worker after future emergency halts is
a local DevOps acknowledgement and does not require governance to clear the halt.
Publishing this release does not enact an upgrade or enable a worker. Previously
collected approvals can still allow a request to settle, so audit pending requests
before activation.

The release download includes an unsigned `system.setCode` preimage and its
`preimage.notePreimage` call, with the finalized metadata and hashes used to encode
them. These are encoding artifacts, not submitted transactions or a claim that a
governance proposal has been approved.

## Build input and validation

Use tag `4.8.15`, the repository's pinned `nightly-2026-10-23` toolchain, and a
dedicated build checkout. The mainnet build requires the external verified recovery
policy file with SHA-256
`b0a2c026fea1c33b524f8e0454881d5ab4cb94ae47a8d09fb59dc97ea3cb7a85`.
Never commit that generated policy. The repository's empty policy is not a mainnet
release input. Source paths and build configuration can affect Wasm bytes; a rebuild
must reproduce the recorded code hash or be reviewed as a different candidate.

Release evidence records the clean source commit and tree, external policy hash,
toolchain, build inputs, artifact checksums, and focused native and actual-Wasm
regressions. The final build verifies that producing the node did not change either
Wasm blob. Test profiles do not replace production build profiles.

The Docker image contains the separately verified Linux amd64 node, runs as
UID/GID 10000, and contains no deployment configuration or signer material. Its
source revision, binary checksum, package inventory and exact image digest are
recorded. Publishing tag 4.8.15 skips the legacy Jenkins image publisher; Jenkins
also no longer invokes automatic downstream runtime deployments.
