# SORA Nexus legacy recovery bootstrap

The bootstrap is built from deployed source tree `36c466398e3f2760e8ab44fdb89de796f5b0dd99`, runtime 134 / transaction 131 and `polkadot-stable2603-3`. It is **not** runtime 138 relabelled as 134. Node 4.8.13 and final runtime 138 use stable2609 independently.

`prepare.py` exports only tracked build sources into a new directory. It preserves the original dependency lock, runtime composition, existing call indices, signed extensions and ordinary executive. The reviewed overlay delegates ordinary block execution through block 28,191,817. Above that height it runs a separate executive with exactly one timestamp inherent and one versioned recovery envelope transported inside a bare `System::remark`. No ordinary runtime upgrade, session, staking, authorship, scheduler, idle or offchain hooks run during maintenance. BABE and timestamp hooks are invoked explicitly. Calls to `System::remark` outside this executive never execute recovery work.

The node supplies envelopes from `--recovery-inherent-dir`; files are named `0x<PARENT_HASH>.scale` and contain `RecoveryInherent::encode_remark()` bytes. An absent file produces a Tick. Runtime policy commitments and council signatures authorize the contents; the directory is not an administrative authority. The runtime rejects signed/general extrinsics, extra inherents and ordinary economic calls. Node import and authoring additionally bind escape signatures to the executing substitute hash.

## Development exports

```sh
python3 misc/incident_recovery/bootstrap/prepare.py --destination /absolute/new/source-directory --spec-version 134
python3 -m unittest discover -s misc/incident_recovery/bootstrap -p 'test_*.py' -v
```

Without `--binding`, the export can compile and replay preserved history but rejects the first recovery child. It cannot be published as an executable recovery release. The source provenance always says `releaseReady: false`; only the outer release gate may admit artifacts after exact-artifact rehearsals.

Use the pinned compiler in the exported `rust-toolchain.toml` and an existing warm Cargo lane. Do not run `cargo clean`. Adding the direct `sp-trie` dependency may update the runtime package's dependency list in Cargo.lock; review that diff and reject package-version drift. Record the resulting lock and compiler hashes separately from the preserved original lock.

To refresh an untouched **unbound** development export:

```sh
python3 misc/incident_recovery/bootstrap/prepare.py --destination /absolute/existing/development-directory --refresh-overlay
```

Refresh verifies previous overlay hashes before replacing the explicit overlay files. It preserves Cargo.lock and the build target, refuses bound exports and records the old provenance plus retained lock hash. Locally edited overlay files cause a refusal. A bound artifact must instead be reproduced in a fresh source export.

## Binding and build order

Policy format 2 binds a bounded ASCII chain-spec ID and a nonzero 32-byte public
release nonce before any Wasm is built. Forward-upgrade signatures use the v2
message domain and include this identity in the authorization. Runtime and native
verification reject a different network, nonce or original policy; authorized
repairs cannot change the identity. Production and rehearsal signatures are never
interchangeable. Old policy-v1 artifacts require a fresh build, not a byte patch.
The ordinary executive through the checkpoint is unchanged.

The first recovery child normally carries Tick and cannot execute Escape. Its
hash is only available afterward. On initializing the second child, the executive
persists the first child's full parent hash under a protected recovery key. Every
escape/forward-repair authorization additionally signs that exact `recoveryAnchor`;
zero, missing or different anchors fail. A frozen first-child chunk can therefore
be rescued from its second child. A first-child failure before producing a valid
block remains outside this escape mechanism's protection.

Exact-artifact offline rehearsals have the same policy/domain as the intended
release, so domain separation alone is insufficient. Their unsigned first-child
headers deliberately contain no BABE seal; any valid production first-child
header contains its seal and has a different hash. Rehearsal signatures must bind
only the observed unsealed rehearsal anchor. Never collect or reuse rehearsal
signatures for the sealed production anchor. Production authorizations are signed
only after the actual first recovery block is known. The native replay and release
verifier derive the anchor from executed block bytes and reject first-child escapes.

The public binding JSON has exactly these fields: `policyScale`, `policyCommitment`, `operatorsScale`, `previousGrandpaSetId`, `firstRecoverySlot`, `finalWasm`; spec 134 additionally requires `holdWasm`. Policy/operator values are SCALE bytes encoded as hexadecimal. Artifact paths are relative to the binding file. The release compiler must validate their schemas, signatures, complete storage coverage, all authority inventories and all required release evidence before providing this binding. Source preparation checks the binding hash but is not a substitute for that compiler.

`operatorsScale` encodes `Vec<OperatorReplacement>` from `incident_recovery_consensus.rs`. Each record contains its owner, a sorted `retired_grandpa` bounded vector of at most three keys (the exact union of active, queued and NextKeys GRANDPA keys), and one fresh GRANDPA key, followed by sorted approved BABE and im-online key unions (each bounded to three). The runtime verifies ownership in each checkpoint roster, rejects omitted or unused retirement keys, requires the retained-key unions to match the approved inventory exactly, and replaces all three GRANDPA paths. Approved independent session keys remain intact. The chain specification's complete retired/approved inventories must match these records, including queued operators absent from the active GRANDPA set.

Build final 138 from its own bound stable2609 source, then hold 135, then bootstrap 134. The bootstrap embeds the already-built hold bytes, so an escape does not depend on earlier successful staging. Hold does not embed its own hash. Final bytes are delivered in 64 KiB chunks, each checked against a compiled hash list before writing. Final/hold cursors and chunks are keyed by the compiled artifact hash: a quorum-authorized repair that pins a corrected 138 gets a separate empty cursor, and cannot assemble leftover bytes from a previous final artifact. Ordinary envelopes cannot choose that hash or reset a pinned artifact cursor. Repair chunks require a strict-majority council authorization before the first write. The authorized code hash names an immutable chunk namespace; ordinary envelopes cannot overwrite its bytes or reset its cursor. A different approved repair has its own namespace. A later quorum may resume an already approved artifact, but cannot change its length or chunk commitments. The outer manifest binds all artifacts and avoids a circular Wasm hash.

For repair exports 136/137, also provide `originalPolicyScale` and `originalPolicyCommitment`. Forward repair may revise committed work/audit data while preserving the original checkpoint, account mapping, recovered council, authorization domain and permanent lineage. `Escape.work_policy` carries the revised policy, bound by the signed action. The original council authorizes a monotonic destination version and current stored/executing code hashes. Installing the revised work policy, incrementing its replay counter and installing the destination code occur in one transaction.

Repair staging uses appended action **12**, `AuthorizeRepairStaging`, preserving the earlier action indices. Its SCALE body is `RepairStagingManifest` followed by `Vec<(AccountId32, MultiSignature)>`. The manifest is the existing `RepairAuthorization`, then a `u64` staging nonce, a `u32` code length and `Vec<[u8; 32]>` ordered chunk hashes. The code is bounded to 32 MiB, split at 64 KiB, with an exact final-chunk length. Signers authorize SCALE `(b"sora-nexus/recovery-repair-staging/v1" as bytes, genesis hash, checkpoint hash, manifest)`; the later Escape retains its separate forward-upgrade signing domain. Neither signature can authorize the other action. The independent staging nonce increases once per accepted manifest; the existing Escape operation counter changes only when installing code.

Prepare the unsigned signing material with `incident-recovery repair-staging-payloads --authorization repair-authorization.scale --code repair.wasm --staging-nonce N`. This command binds the exact bytes and emits `manifestScale` and `signingMessageScale`; it does not sign, verify runtime identity, infer the live nonce or assert production readiness. The quorum-signed action is encoded inside the usual recovery envelope as identifier, format version 1, action index 12, manifest, signatures. Retain the signed envelope, then stage only the exact committed chunks and submit the independently signed Escape. Public rehearsal fixtures use public fixture keys and the observed unsealed first-child anchor; never reuse production council signatures in a fixture.

The replay checks the signed staging action against the original recovered council, actual stored code, recovery anchor, status and nonce, then verifies the retained authorization, unchanged economic cursor/counter, independent import and disk restart. It retains `<block>.repair-staging.{manifest.scale,signing-message.scale,envelope.scale,authorization.json}` and reports `repairStagingAuthorizations`. An unsigned sidecar is not execution evidence. Previously built runtimes and old fixtures without action 12 do not establish this new staging behavior.

If a repair's compiled policy does not match its pending handover, initialization preserves the original lineage/council, cursor and operation counter, records Failed and keeps the next permitted council repair available. Ordinary work remains frozen. Fundamental invalid bootstrap policy/lineage and failures before this handler executes remain outside its protection.

Automatic activation requires the independent terminal audit to reach Ready, the complete pinned final bytes, valid runtime 138 / transaction 134 identity, preserved fresh-key ownership and maintenance epoch attribution, and the bounded Kensetsu fee-clock preconditions. It records Complete before installing 138, so the next block's authoring provider stops inserting maintenance envelopes. Millisecond checkpoint/activation timestamps are retained for the final runtime's one-time fee-clock shift.

## Compatibility and release evidence

The source-only bridge patch is applied with `git apply --check`; it does not copy newer SDK manifests into the historical build. Existing bridge dispatch APIs were confirmed in the pinned source. The legacy bridge types retain their deployed Derivative implementation. Cached HASHI transactions retain deployed signer selection; only manifest-recovery quarantine validation is added. The final runtime separately carries the upgraded peer signer selection. Added error variants and recovery branches can change metadata bytes, so clients must select metadata by executing code identity. Existing call/error indices must remain stable. Exact pre-checkpoint **state-root replay** remains the primary compatibility gate; source equivalence is supporting evidence only.

Required tests still include exact retained session/epoch/upgrade boundary replay, full and warp sync across the fresh GRANDPA set, preserved and fresh-key equivocation proofs, maintenance epoch attribution and deferred session resumption, every chunk/audit/activation and crash restart, failed-chunk escape, signed forward repair/replay rejection, authenticated incoming consumption and deferred acknowledgements, and post-activation bridge halt. No production binding, signing material, deployed service change or restart is created by these tools.
