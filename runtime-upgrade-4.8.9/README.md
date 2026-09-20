# SORA runtime 4.8.9 — complete council package

This package includes the latest payout, bridge, liquidation, validator liveness,
session-boundary authorship and deferred-slashing changes. It is built and
validated for upgrading SORA mainnet from spec 130 to spec 131, transaction 131.
All governance calls are unsigned; nothing has been submitted to mainnet.

Start with **[COUNCIL-PROPOSAL.md](COUNCIL-PROPOSAL.md)**, the forwardable council
brief, and **[GOVERNANCE-RUNBOOK.md](GOVERNANCE-RUNBOOK.md)** for submission,
explicit votes, closing motions, referendum and enactment checks.

## Exact candidate

| Item | Value |
| --- | --- |
| Release commit | `823a5b9fde8486dd73aeab856c76bbab5408bf0f` |
| Wasm | `framenode-runtime-4.8.9.compact.compressed.wasm` |
| Wasm size | 3,055,506 bytes |
| Wasm SHA-256 | `db948406c5f22d4923b2760019de53bcd0ef756ed05accaf5156ca3041988447` |
| `System.set_code` proposal hash | `0x6de23331a9c21a5ca06944b6cce28c0c0304c57ecd9b26af9fe2bb413b213b04` |
| Proposal length | 3,055,512 bytes |

This replaces the earlier payout/bridge-only package. Do not submit its previous
proposal hash `0xcf83851e197e1597ded8ac24f7b9e5141c3db641ba8a7f61b602b3767e4c1420`.

## Included files

- `COUNCIL-PROPOSAL.md`: proposed change, governance parameters, scope and limits.
- `GOVERNANCE-RUNBOOK.md`: preimage, council, technical committee and referendum steps.
- Wasm and `framenode-runtime-4.8.9-metadata.json`: exact deployable binary and metadata.
- `set-code-call.scale` / `.hex`: complete runtime proposal bytes for the preimage.
- `preimage-note-call.hex`: complete outer preimage registration call.
- Council threshold 4/5 and technical committee threshold 3 encoded proposal calls.
- `governance-calls.json`, `preimage.json`: exact call hashes and lengths.
- `VALIDATION.md` and `validation/`: test logs, metadata comparison, native snapshot,
  compiled-Wasm replay reports, scripts and source hashes.
- `source.tar.gz`: full source at the release commit, including all new files.
- `source-changes.bundle`: Git commit/history bundle relative to the recorded base.
- `polkadotjs-val-payouts.patch`: companion frontend patch, requiring separate publication.
- `runtime-upgrade-4.8.9-info.json`: machine-readable release manifest.
- `SHA256SUMS`: checksums for every package file.

## Verify before signing

From this package directory:

```sh
python3 validation/verify-package.py
```

This checks the exact Wasm, SCALE proposal, seven unsigned calls and every file
listed in `SHA256SUMS`. The proposal hash includes the encoded `System.set_code`
call; it is different from the raw Wasm hash.

## Source and rebuild

The release commit is included locally in the bundle; it has not been pushed to
a remote repository. The full source archive needs no Git prerequisite. To
inspect the Git commit in a clone that contains base commit
`7104c2ff6e8d5c2babfc7743e8e5851ba65e5a58`:

```sh
git bundle verify /path/to/source-changes.bundle
git fetch /path/to/source-changes.bundle refs/heads/codex/runtime-4.8.9-council-823a5b9f
git switch --detach 823a5b9fde8486dd73aeab856c76bbab5408bf0f
```

Alternatively, extract `source.tar.gz` into a new empty directory. Build using
Rust `nightly-2025-05-08` and the repository's normal WasmBuilder:

```sh
env -u SKIP_WASM_BUILD \
CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm@21/bin/clang \
AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm@21/bin/llvm-ar \
LIBCLANG_PATH=/opt/homebrew/opt/llvm@21/lib \
LLVM_CONFIG_PATH=/opt/homebrew/opt/llvm@21/bin/llvm-config \
scripts/with_llvm_env.sh cargo build --release --locked \
  -p framenode-runtime --features build-wasm-binary
```

The LLVM paths above record this Mac's build environment. The SDK selected
`wasm32v1-none`. Default mainnet features and `build-wasm-binary` are enabled;
`wip`, `private-net`, `stage`, benchmarks and try-runtime are disabled in the
on-chain blob. This runtime-only package does not contain a node executable or
container image.

The original shared checkout and its index were preserved. The release source
commit captures the complete changes; source hashes were checked after tests.
The native and Wasm rehearsals passed, with their coverage and limits recorded
in `VALIDATION.md`. The four ignored runtime tests remain explicitly listed.
