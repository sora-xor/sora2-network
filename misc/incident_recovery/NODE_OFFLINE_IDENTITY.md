# SORA Nexus offline node identity and prepared template

`node_identity.py` identifies a supplied native node executable and reproduces
the initial recovery chain-spec template using that executable. It does not
start a node, open a chain database, contact an RPC endpoint, use operator
credentials, modify a service, or approve network discovery addresses.

The invoking operator must independently approve the executable. An SHA-256
pin establishes byte identity; a version string and successful CLI invocation
do not authenticate the source or build. The tool uses a minimal fixed
environment with scratch TMP/XDG directories and omits HOME/CODEX_HOME. It does
not install an operating-system network sandbox.

The manifest schema is `sora-nexus-node-offline-identity-v1`. Its only other
field is `artifacts`, an object with exactly four roles: `nodeBinary`,
`chainSpec`, `recoveryConfig`, and `bootstrapWasm`. Each binding has exactly
`file` (canonical relative path under the manifest directory), `sha256`
(lowercase hexadecimal SHA-256), and `bytes` (exact integer byte length).
Unknown fields, commands, environments, readiness flags, scripts, symlinks,
hardlinks, credential paths, mismatching pins and existing outputs are rejected.
The node must be a native ELF or Mach-O file with an executable permission bit.
Both original inputs and fresh copies are rehashed around execution.

Run from the reviewed verifier source directory:

```text
python3 node_identity.py --manifest /absolute/public-inputs/node-identity.json --output /absolute/new-offline-result
```

The fixed command sequence, using a freshly copied executable named `framenode`,
is:

```text
framenode --version
framenode prepare-recovery-spec --recovery-config recovery-config.json --bootstrap-wasm bootstrap.wasm --output prepared.json
```

The version output must be exactly a `framenode 4.8.13-...` line with the normal
CLI newline suffix. Its complete bytes and digest are retained. The preparation
command uses the node's existing `PrepareRecoverySpecCmd::run`, which invokes
the typed recovery configuration checks and `recovery::validate_spec`, including
duplicate substitute versions and the spec-134/transaction-131 bootstrap binding.
`build-spec` is deliberately insufficient for this check: its command path
does not invoke those recovery checks.

The supplied recovery config must equal the chain-spec extension, the checkpoint
substitute must equal the pinned Wasm bytes, and the prepared output must equal
the entire supplied chain spec after strict JSON parsing. Comparison is type
sensitive; object ordering and whitespace may differ. This includes the raw
genesis, retained substitutes, recovery extension, protocol and fork identities.
The initial template must have **empty bootnode and telemetry lists**. The tool
does not ignore, approve or normalize discovery edits. Later operator discovery
configuration needs an explicit reviewed binding and separate admission checks.

The result retains executable/input hashes, raw stdout/stderr, process IDs,
fixed argument arrays, working directory, explicit environment, exit status,
timing and output digests. Launch intent and then the actual PID are flushed to
disk before waiting, so an interruption retains the partial process evidence.
Each run creates a new output directory. Failures
after creation preserve diagnostics and do not write `result.json`.

`executableVersionObserved` and `exactOfflineTemplateReproduced` describe the
bounded successful check. Source/build authenticity, operator inventory approval,
production network admission, database startup and consensus readiness remain
unverified. `releaseGatePromoted` and `productionReady` are always false.
This command does not establish checkpoint finality, full/warp synchronization,
startup guards against actual databases or keystores, retained execution roots,
runtime escape behavior, killed-process restart behavior, or live readiness.

The focused tests mock native processes and use visibly synthetic bytes. They
verify verifier logic only. They are not evidence that a release node or chain
spec has passed the real offline command sequence.

The fixed `prepare-recovery-spec` command retains the SDK default SS58 prefix42.
The four GRANDPA AuthorityId JSON fields must use canonical prefix42 SS58 strings;
H256 commitment fields remain hex. The checker validates that encoding before
launch, and the native typed parser still validates authority semantics. Keep
inventory order based on raw32-byte keys. Do not relax whole-template equality
to accommodate alternate serialized prefixes or invalid hex public-key strings.

Bootstrap input may be raw Wasm or SDK-prefixed Zstandard Wasm. Compressed input
must contain one complete frame with no trailing bytes, decode to the Wasm header,
and stay within the SDK 50MiB decoded-code bound; the decoder window is capped at
128MiB. This format preflight does not execute Wasm. The exact supplied bytes are
copied, hashed and compared with the checkpoint substitute, including when they
are compressed. Decompression never normalizes the committed artifact identity.
Use the same representation as the exact-artifact rehearsal; a raw134 template
check does not establish the identity of a different compressed134 artifact.
