# SORA Nexus fresh storage-semantics evidence

This optional release adapter produces useful executable evidence and deliberately leaves `completeStorageSemantics` **unverified**. It does not claim complete historical control discovery, complete beneficial ownership/debt attribution, retired-key dispatch coverage, consensus or release readiness.

Add the public artifact role `storageCompositionInputs` to the candidate. Its bytes must be the original `sora-nexus-production-recovery-build-input-v1` manifest used to compile the policy. The canonical manifest commitment must equal the package policy's evidence commitment. Its exact file inventory must include every captured trie page and all typed composition inputs. CSV, ownership proofs, semantic plan, derived graph, operators, bridge signing/HASHI and staking role reviews join their package roles. Public fixture markers/keys, unsafe paths and modified/missing files fail admission. The existing exact nonfixture `artifactActivationReplay` / `artifactHoldReplay` pair is required for its complete code, capture and consensus bindings. This adapter executes only the normal original-policy activation branch; repair-policy state semantics require a separate revalidation path.

With the ordinary pinned `--evidence-verifier` arguments, packaging now performs these fresh commands in its owned scratch graph:

1. `verify-ownership`, then `compose-storage-work` with every committed input and operator inventory.
2. `compile-mixed-work` if the original committed layout calls for it. Every composed operation/audit remains exactly once in original order; derived action10 audits retain the original baseline.
3. `emit-final-policy`. The exact resulting policy and ordered work/audit/terminal envelope bytes must equal the package activation inputs.
4. `rehearse-storage-semantics --manifest OWNED_ACTIVATION_MANIFEST --output FRESH_REPLAY_DIRECTORY`.

Composition stdout has a finite 512 MiB bound (the full retained composition is larger than 128 MiB). Ordinary process limits remain unchanged. Composition allows six hours; replay allows 24 hours. The executable, all source helpers, complete input graph, process receipts, metadata, blocks and snapshot objects remain pinned through atomic artifact registration. Failure retains its scratch evidence and registers no successful result.

The new native command runs the existing author/import/persisted-restore path. Only a normal nonfixture 134→138 replay is admitted. Its same-process final externalities must match the last imported block, contain and have executed exact final138/tx134, and remain unchanged by verification. It executes `Metadata_metadata` using that actual final Wasm, retains the returned SCALE metadata, and scans **all top storage and child tries** using that schema. It rejects typed old owners in `System.Account` and `Tokens.Accounts`. Other typed old references, unknown legacy layouts, opaque hash-only map keys and child layouts lacking final metadata are reported as unresolved; no substring scan or historical label substitutes for type evidence.

A new `cfg(feature = "std")` helper rechecks the exact completed recovery receipt, work/audit cursors, zero pending incoming audits, permanent halt/transient-context conditions, committed issuance equations, collateral minima, extinguished claims and absent ownership rows. It reads the same invariant code used at activation and performs no storage writes. It does not change no_std runtime behavior. A legitimate post-activation migration that changes a guarded record will fail this narrow recheck until an explicit typed migration-aware projection is implemented; changing a report flag cannot waive it.

The report separates concrete evidence from missing proof:

- The full final trie was enumerated; only entries decoded by the executing final metadata are typed.
- Committed financial and absence witnesses were rechecked; those witnesses do not establish that all financial liabilities were included.
- Actual policy/runtime/state identities are joined; the supplied derived graph does not prove that all derived accounts were found.
- Old liquid-account rows are absent; runtime-wide rejection of every retired signing/control path remains a distinct test obligation.

The missing native proof interface should be a private, non-`Deserialize` certificate constructed from authenticated complete available pre-checkpoint history for every mapped root. It must bind checkpoint/hash, complete block/body/event and executing-metadata coverage, exact mapping and new-owner control confirmations; discover utility derivatives (including intermediate/nested derivations), completed and pending multisig controls, and pre-compromise grants/payout changes. A second final-schema adapter must consume that certificate plus freshly re-executed compiler resolutions and the same final externalities to classify allowed immutable references, verify mutable grants/ownership and lifecycle backing, and establish full debt/shared-position coverage. Missing blocks, unknown codecs or unresolved beneficial interests must keep the gate false. Neither interface accepts success JSON or a caller's completeness assertion.

Local diagnostic command after building this utility:

```text
incident-recovery rehearse-storage-semantics --manifest /ABSOLUTE/FRESH/NORMAL-ACTIVATION.json --output /ABSOLUTE/NEW/STORAGE-REPLAY
```

The manifest must be freshly assembled from the full composition and actual new Wasm/native artifacts, and contain every apply/audit/terminal step plus at least one block executed by runtime138. A complete composition with 739 apply and 1,144 audits cannot be replaced by the old tiny fixture. Public-key diagnostic runs never enter the production package adapter; operator/signed replacement data remain required release inputs.
