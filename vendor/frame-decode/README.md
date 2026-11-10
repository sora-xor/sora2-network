# frame-decode

Decode extrinsics, storage keys and storage values from modern or historic Substrate runtimes.

See https://docs.rs/frame-decode/latest/frame_decode/ for more documentation and examples.

# Examples

## Decoding historic extrinsics

```rust
use frame_decode::extrinsics::decode_extrinsic_legacy;
use frame_metadata::RuntimeMetadata;
use parity_scale_codec::Decode;
use scale_info_legacy::ChainTypeRegistry;
use scale_value::scale::ValueVisitor;

let metadata_bytes = std::fs::read("artifacts/metadata_5000000_30.scale").unwrap();
let RuntimeMetadata::V12(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { panic!() };

let extrinsics_bytes = std::fs::read("artifacts/exts_5000000_30.json").unwrap();
let extrinsics_hex: Vec<String> = serde_json::from_slice(&extrinsics_bytes).unwrap();

// For historic types, we also need to provide type definitions, since they aren't in the
// metadata. We use scale-info-legacy to do this, and have already defined types for the
// Polkadot relay chain, so let's load those in:
let historic_type_bytes = std::fs::read("types/polkadot_types.yaml").unwrap();
let historic_types: ChainTypeRegistry = serde_saphyr::from_slice(&historic_type_bytes).unwrap();

// We configure the loaded types for the spec version of the extrinsics we want to decode,
// because types can vary between different spec versions.
let mut historic_types_for_spec = historic_types.for_spec_version(30);

// We also want to embelish these types with information from the metadata itself. This avoids
// needing to hardcode a load of type definitions that we can already construct from the metadata.
let types_from_metadata = frame_decode::helpers::type_registry_from_metadata(&metadata).unwrap();
historic_types_for_spec.prepend(types_from_metadata);

for ext_hex in extrinsics_hex {
    let ext_bytes = hex::decode(ext_hex.trim_start_matches("0x")).unwrap();

    // Decode the extrinsic, returning information about it:
    let ext_info = decode_extrinsic_legacy(&mut &*ext_bytes, &metadata, &historic_types_for_spec).unwrap();

    // Decode the signature details to scale_value::Values.
    if let Some(sig) = ext_info.signature_payload() {
        let address_bytes =  &ext_bytes[sig.address_range()];
        let address_value = decode_with_visitor(
            &mut &*address_bytes,
            *sig.address_type(),
            &metadata.types,
            ValueVisitor::new()
        ).unwrap();

        let signature_bytes = &ext_bytes[sig.signature_range()];
        let signature_value = decode_with_visitor(
            &mut &*signature_bytes,
            *sig.signature_type(),
            &metadata.types,
            ValueVisitor::new()
        ).unwrap();
    }

    // Decode the transaction extensions to scale_value::Values.
    if let Some(exts) = ext_info.transaction_extension_payload() {
        for ext in exts.iter() {
            let ext_name = ext.name();
            let ext_bytes = &ext_bytes[ext.range()];
            let ext_value = decode_with_visitor(
                &mut &*ext_bytes,
                *ext.ty(),
                &metadata.types,
                ValueVisitor::new()
            ).unwrap();
        }
    }

    // Decode the call data args to scale_value::Values.
    for arg in ext_info.call_data() {
        let arg_name = arg.name();
        let arg_bytes = &ext_bytes[arg.range()];
        let arg_value = decode_with_visitor(
            &mut &*arg_bytes,
            *arg.ty(),
            &metadata.types,
            ValueVisitor::new()
        ).unwrap();
    }
}
```

## Decoding historic storage keys

```rust
use frame_decode::storage::decode_storage_key_legacy;
use frame_metadata::RuntimeMetadata;
use parity_scale_codec::Decode;
use scale_info_legacy::ChainTypeRegistry;
use scale_value::scale::ValueVisitor;

let metadata_bytes = std::fs::read("artifacts/metadata_5000000_30.scale").unwrap();
let RuntimeMetadata::V12(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { panic!() };
 
let storage_keyval_bytes = std::fs::read("artifacts/storage_5000000_30_staking_validators.json").unwrap();
let storage_keyval_hex: Vec<(String, String)> = serde_json::from_slice(&storage_keyval_bytes).unwrap();

// For historic types, we also need to provide type definitions, since they aren't in the
// metadata. We use scale-info-legacy to do this, and have already defined types for the
// Polkadot relay chain, so let's load those in:
let historic_type_bytes = std::fs::read("types/polkadot_types.yaml").unwrap();
let historic_types: ChainTypeRegistry = serde_saphyr::from_slice(&historic_type_bytes).unwrap();

// We configure the loaded types for the spec version of the extrinsics we want to decode,
// because types can vary between different spec versions.
let mut historic_types_for_spec = historic_types.for_spec_version(30);

// We also want to embelish these types with information from the metadata itself. This avoids
// needing to hardcode a load of type definitions that we can already construct from the metadata.
let types_from_metadata = frame_decode::helpers::type_registry_from_metadata(&metadata).unwrap();
historic_types_for_spec.prepend(types_from_metadata);

for (key, _val) in storage_keyval_hex {
    let key_bytes = hex::decode(key.trim_start_matches("0x")).unwrap();

    // Decode the storage key, returning information about it:
    let storage_info = decode_storage_key_legacy(
        "Staking",
        "Validators",
        &mut &*key_bytes,
        &metadata,
        &historic_types_for_spec
    ).unwrap();

   for part in storage_info.parts() {
       // Access information about the hasher for this part of the key:
       let hash_bytes = &key_bytes[part.hash_range()];
       let hasher = part.hasher();

       // If the value is encoded as part of the hasher, we can find and
       // decode the value too:
       if let Some(value_info) = part.value() {
           let value_bytes = &key_bytes[value_info.range()];
           let value = decode_with_visitor(
               &mut &*value_bytes,
               *value_info.ty(),
               &metadata.types,
               ValueVisitor::new()
           ).unwrap();
       }
   }
}
```

# Decoding historic storage values

```rust
use frame_decode::storage::decode_storage_value_legacy;
use frame_metadata::RuntimeMetadata;
use parity_scale_codec::Decode;
use scale_info_legacy::ChainTypeRegistry;
use scale_value::scale::ValueVisitor;

let metadata_bytes = std::fs::read("artifacts/metadata_5000000_30.scale").unwrap();
let RuntimeMetadata::V12(metadata) = RuntimeMetadata::decode(&mut &*metadata_bytes).unwrap() else { panic!() };
 
let storage_keyval_bytes = std::fs::read("artifacts/storage_5000000_30_staking_validators.json").unwrap();
let storage_keyval_hex: Vec<(String, String)> = serde_json::from_slice(&storage_keyval_bytes).unwrap();

// For historic types, we also need to provide type definitions, since they aren't in the
// metadata. We use scale-info-legacy to do this, and have already defined types for the
// Polkadot relay chain, so let's load those in:
let historic_type_bytes = std::fs::read("types/polkadot_types.yaml").unwrap();
let historic_types: ChainTypeRegistry = serde_saphyr::from_slice(&historic_type_bytes).unwrap();

// We configure the loaded types for the spec version of the extrinsics we want to decode,
// because types can vary between different spec versions.
let mut historic_types_for_spec = historic_types.for_spec_version(30);

// We also want to embelish these types with information from the metadata itself. This avoids
// needing to hardcode a load of type definitions that we can already construct from the metadata.
let types_from_metadata = frame_decode::helpers::type_registry_from_metadata(&metadata).unwrap();
historic_types_for_spec.prepend(types_from_metadata);

for (_key, val) in storage_keyval_hex {
    let value_bytes = hex::decode(val.trim_start_matches("0x")).unwrap();

    // Decode the storage value, here into a scale_value::Value:
    let account_value = decode_storage_value_legacy(
        "Staking",
        "Validators",
        &mut &*value_bytes,
        &metadata,
        &historic_types_for_spec,
        ValueVisitor::new()
    ).unwrap();
}
```

## `frame-decode-tester` (CI decoding tests)

This repo includes an integration-test crate, `frame-decode-tester`, which validates that `frame-decode`
can decode **historic extrinsics** and **historic storage values** against live chains.

### What these tests do

- **Historic block decoding**: fetches the block body (extrinsics) for selected historic blocks and attempts
  to decode each extrinsic into `(pallet.call, args)` using chain metadata + the historic type registry.
- **Historic storage decoding**: for selected historic blocks, fetches keys under one or more pallet/storage
  prefixes and attempts to decode the corresponding values.

These tests use **live public RPC endpoints**, so they include retry/backoff and are designed to keep RPC load
reasonable (multiple URLs + modest concurrency).

### Current test coverage

- **Kusama Asset Hub**
  - `kusama_assethub_historic_block`: decodes extrinsics across blocks around runtime upgrades.
  - `kusama_assethub_historic_storage`: decodes storage values across the same block set.
- **Kusama Relay**
  - `kusama_relay_historic_block`: decodes extrinsics across blocks around runtime upgrades.
  - `kusama_relay_historic_storage`: decodes storage values across the same block set.

The block list is based on “spec version change markers” (first block under a new runtime spec), and we
currently test **3 consecutive blocks per marker**: `b, b+1, b+2`. For historic coverage we cap markers
to **pre-V14 transitions** (V14+ metadata embeds types).

### Test tiers (PR vs deep)

Tests support a tier switch via `FRAME_DECODE_TIER`:

- **Default / PR tier**: `FRAME_DECODE_TIER` unset (or anything except `deep`) → PR mode.
- **Deep tier**: `FRAME_DECODE_TIER=deep` → deeper settings where supported.

The default is intentionally PR tier so local runs are predictable even if the env var is missing.

**Note:** tiers control **sampling sizes / storage breadth** (e.g. how many blocks between spec changes, how many keys per entry, etc.) to target ~5 min (PR) vs ~hour long (deep) runs.

### How these run in CI

GitHub Actions workflow: `.github/workflows/rust.yml`

The `decode-tester` CI job is gated using `dorny/paths-filter` and only runs when relevant files change, e.g.:
- `types/kusama_assethub_types.yaml`
- `frame-decode-tester/src/**`
- `frame-decode-tester/tests/kusama-assethub-*.rs`
- `frame-decode-tester/Cargo.toml`

When the gate is triggered, CI runs the decode tests in **deep tier** by default to ensure maximum coverage before merge.

### Benchmark/throughput reporting

Each test run emits a `METRIC` line summarizing throughput:
- `METRIC decode_blocks ... secs=... blocks_per_s=... extrinsics_per_s=...`
- `METRIC decode_storage ... secs=... blocks_per_s=... values_per_s=...`

These help guide tuning of concurrency and sampling parameters.

### How to run locally

#### Run Kusama Asset Hub decode tests (PR tier / default)

```bash
cargo test -p frame-decode-tester --features kusama-assethub \
  --test kusama_assethub_historic_block \
  --test kusama_assethub_historic_storage \
  -- --nocapture
```

#### Run Kusama Asset Hub decode tests (deep tier)

```bash
FRAME_DECODE_TIER=deep cargo test -p frame-decode-tester --features kusama-assethub \
  --test kusama_assethub_historic_block \
  --test kusama_assethub_historic_storage \
  -- --nocapture
```

#### Enable debug logs

Set `FRAME_DECODE_TEST_DEBUG=1` (or `true`) to print a short per-run summary (tier, concurrency, counts) and a
few sample block/spec-version lines:

```bash
FRAME_DECODE_TEST_DEBUG=1 FRAME_DECODE_TIER=pr cargo test -p frame-decode-tester --features kusama-assethub \
  --test kusama_assethub_historic_block \
  --test kusama_assethub_historic_storage \
  -- --nocapture
```

#### Run all Kusama historic tests (Asset Hub + Relay)

```bash
FRAME_DECODE_TEST_DEBUG=1 FRAME_DECODE_TIER=pr cargo test -p frame-decode-tester --features "kusama-assethub kusama-relay" \
  --test kusama_assethub_historic_block \
  --test kusama_assethub_historic_storage \
  --test kusama_relay_historic_block \
  --test kusama_relay_historic_storage \
  -- --nocapture
```

### RPC endpoints + rate limiting notes

Public RPC endpoints may rate-limit (e.g. HTTP 429 during WebSocket connection establishment). To improve
stability, tests typically:
- use a small list of RPC URLs and spread work across them
- keep concurrency modest (especially for storage tests)
- retry with backoff on transient RPC failures

If you see flakiness, try rerunning with PR tier defaults and/or reducing concurrency.

Storage decoding is particularly RPC-heavy (key enumeration + per-key value fetch). For this reason, the
storage tester includes per-request retry/backoff and can fail over to another RPC URL on transient errors.
