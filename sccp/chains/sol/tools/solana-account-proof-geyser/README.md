# `solana-account-proof-geyser`

Reference validator-side Geyser plugin for the Solana -> SORA proof path.

It watches:

- the configured SCCP burn-record PDA set
- the `SlotHashes` sysvar account
- transaction signature counts
- block metadata
- slot status transitions

For each confirmed slot that modified one of the monitored accounts, it computes the Solana account-delta Merkle root, assembles the monitored-account inclusion proofs, computes the bank hash, and streams a Borsh-encoded `Update` over TCP.

The companion builder at [`../solana-finalized-burn-witness/`](../solana-finalized-burn-witness) consumes that stream and turns it into SCALE(`SolanaFinalizedBurnProofV1`) bytes for SORA.

## Config

Example plugin config JSON:

```json
{
  "libpath": "/absolute/path/to/libsolana_account_proof_geyser.dylib",
  "bind_address": "127.0.0.1:7000",
  "account_list": [
    "BurnRecordPda11111111111111111111111111111111"
  ]
}
```

## Notes

- The plugin automatically adds the `SlotHashes` sysvar account to the monitored set.
- The TCP stream is a raw concatenation of Borsh `Update` values.
- This crate is a reference exporter. The SORA proof builder and verifier do not depend on its exact implementation as long as the exported `Update` schema matches.
