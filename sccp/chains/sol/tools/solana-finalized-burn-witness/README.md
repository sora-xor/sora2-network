# `solana-finalized-burn-witness`

Offline builder for Solana -> SORA finalized burn proof bytes.

This tool consumes:

- Geyser account-proof updates for finalized Solana slots
- the SCCP router program id
- the target `messageId`
- Solana RPC access for fetching the vote block transactions that correspond to later `SlotHashes` witnesses

It produces:

- SCALE(`SolanaFinalizedBurnProofV1`) bytes ready for `sccp.mint_from_proof` / `sccp.attest_burn` on SORA
- optional JSON summary metadata

The proof it builds contains:

- `public_inputs`
- `burn_proof`
- `vote_proofs`

## Inputs

The tool can read witness updates in either mode:

- `--geyser-addr <host:port>` to stream Borsh-encoded updates from a validator-side exporter
- one or more `--update-file <path>` values to replay previously captured Borsh update blobs

If you also provide an authority set JSON file, the tool will wait until enough distinct vote authorities are observed to satisfy the configured or derived threshold.

JSON format:

```json
[
  {
    "authorityPubkey": "Vote111111111111111111111111111111111111111",
    "stake": 123456789
  }
]
```

## Example

```bash
cargo run --manifest-path tools/solana-finalized-burn-witness/Cargo.toml -- \
  build-proof \
  --router-program-id <solana_router_program_id> \
  --message-id 0x<32-byte-message-id> \
  --rpc-url http://127.0.0.1:8899 \
  --geyser-addr 127.0.0.1:7000 \
  --authority-set-json ./vote-authorities.json \
  --json-output ./solana-proof-summary.json \
  --proof-output ./solana-proof.scale \
  --stdout-format both
```

## Notes

- The witness exporter is responsible for supplying the Geyser account-proof updates. This tool does not talk to the validator internals directly.
- The builder filters for the canonical SCCP burn-record PDA derived from `(program_id, messageId)`.
- Vote proofs are derived from raw vote transactions in the slot whose `SlotHashes` sysvar witness includes the burn slot's bank hash.
