# SORA Nexus offline HASHI recovery encoding

`hashi_codec.py` prepares public recovery templates and native signing payloads.
It does not read keys, connect to RPC, submit transactions, infer transfer amounts,
or authenticate signature witnesses. Native runtime admission performs the final
signature, membership, nonce, request, asset and replay checks.

Each template contains the original SCALE `IncomingRequest::Transfer`, exact native
`BridgeMultisig` calls, signer AccountId32 values, native SignedExtra bytes, reviewed
network attacker registries, and the frozen-source evidence commitment. Calls must
come from the executing recovery artifact's metadata, verified against its release
witness. Do not use a spec-version-only metadata cache or assume `:code` identifies
a configured substitute. Unknown senders and ambiguous receipts are rejected by
the runtime; call amounts alone are not external receipt evidence.

Preparation computes the domain-separated operation commitment accepted in the
recovery policy's chunk list. The entire template is immutable after that policy
is built. Native signature bytes alone remain later-supplied witnesses. This avoids
requiring a future recovery block hash while compiling the release.

```sh
python3 misc/incident_recovery/hashi_codec.py prepare \
  --template reviewed-hashi-template.json --output prepared-hashi.json
```

After the first recovery Tick child exists, obtain its actual header and approved
finality evidence. The public signing context supplies exact native spec/transaction
versions, SORA Nexus genesis, execution-code and metadata witnesses, the intended
execution height, and contiguous SCALE headers from checkpoint+1 through an
independently trusted finalized head. Every header must link to the checkpoint and
contain the exact recovery lineage digest. The tool verifies this ancestry and
pinned head hash; it does **not** independently verify GRANDPA finality. The trusted
head and finality-evidence hash must come through the approved operator route.

The committed mortal era must have a birth block after the checkpoint, before the
intended execution block, and present in that ancestry. The source runtime's native
versions and implicit signed fields are required inputs, never guessed defaults.
The runtime checks the actual birth hash from its retained block-hash storage.
Choose a collection/execution window within both the mortal era and native block
hash retention. Expired templates or changed peer nonces require an authorized
forward repair, not rewriting the committed template.

```sh
python3 misc/incident_recovery/hashi_codec.py signing-payloads \
  --template reviewed-hashi-template.json --context verified-context.json \
  --output peer-signing-payloads.json
```

Each output payload matches native `SignedPayload::using_encoded`: the SCALE
call, SignedExtra and implicit fields are hashed with Blake2-256 only when their
combined length exceeds 256 bytes. Operators must use the native account signing
API for their signature scheme. Do not add a wallet text prefix or substitute an
ECDSA prehashed-signing operation for the native signing API. The committed signer
must correspond to the resulting native account identity.

Collect one SCALE `MultiSignature` hex string per committed approval, in exactly
the same order, into a JSON list. Signature tags are the native ed25519, sr25519 or
ECDSA variants. Assemble the witness package without altering its template:

```sh
python3 misc/incident_recovery/hashi_codec.py assemble \
  --template reviewed-hashi-template.json --signatures peer-signatures.json \
  --output hashi-operation-witness.json
```

The output is the `IncomingOperation` SCALE body for `RecoveryAction::Incoming`;
it is not an ordinary signed transaction and is not itself the entire recovery
inherent. Retain the unsigned template, prepared commitment, context/header proof,
finality evidence, signature witnesses and assembled operation in the bound release
and execution evidence. The runtime checks every uncommitted signature before any
state write, so corrupt witness bytes cannot freeze committed work. Valid witness
bytes do not authorize a call absent from the policy.

All output paths are exclusive-create. The tool rejects duplicate JSON keys,
missing evidence, unknown fields, ambiguous network identities, duplicate signers,
immortal eras, nonzero tips, exhausted nonces, and malformed native signature
encodings. It never fills missing approval fields or amounts.

`hashi_codec_fixture.json` is an explicitly synthetic public unit-test fixture
exported by `recovery_hashi_offline_codec_golden` from the native Rust runtime. It
contains no production accounts or keys. Python tests compare its operation SCALE,
commitment and signing payloads byte-for-byte. It must never be a release input.

```sh
python3 misc/incident_recovery/test_hashi_codec.py
```
