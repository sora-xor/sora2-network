# SORA Nexus historical offence preimages

`capture_offence_preimages.mjs` retrieves only the original offence records
needed by the replacement mapping. When a later report did not exist before the
first denomination, `locate_offence_denomination.mjs` finds the next adjacent
cumulative-denominator transition. `capture_offence_preimages_at_anchor.mjs`
then requests only the still-missing report keys immediately before that
transition. The cumulative factors must be positive and compatible with the
checkpoint factor. No inverse arithmetic invents a lost exposure.

Every bounded request and response, including absent records and failures, is
retained. Runs use an explicit request budget and a 15-minute RPC timeout, verify
fixed chain/header identities, preserve read-proof bytes, and reject changed
plans or cached responses. A new run resumes the same immutable capture instead
of replacing its evidence. These observations alone do not verify a historical
state proof or consensus finality.

`classify_offence_preimages_at_anchor.mjs` is offline. It decodes original values,
recomputes the native offence report ID and applies the exact denomination
arithmetic to compare with checkpoint bytes. Missing or contradictory records
remain unresolved. The native composition adapter repeats these checks before
emitting edits and preserves reporter claims, offence counts and historical IDs.
No missing preimage may be replaced by zero exposure or an inferred amount.

The genuine-CSV composition in the ignored `trusted-csv-composition-v8b` evidence
used 1,632 authenticated original preimages. Generated captures, mappings and
composition reports remain outside Git. This result is storage-composition
evidence; release execution and consensus gates are separate.

Run the bounded offline tests with:

```sh
node --test misc/incident_recovery/test_capture_offence_preimages.mjs \
  misc/incident_recovery/test_capture_offence_preimages_at_anchor.mjs \
  misc/incident_recovery/test_locate_offence_denomination.mjs
```
