# SORA Nexus mixed bridge execution fixture

The optional utility feature `bridge-fixture` prepares public synthetic recovery
work on the independently verified actual checkpoint. Its secp256k1 peer seeds
are derived from an explicit `PUBLIC-FIXTURE-ONLY` domain. The generated evidence
does not authenticate a real external transfer or authorize production recovery.

The four committed work steps install exact guarded fixture configuration and
outgoing records, execute a native signed two-message incoming batch, quarantine
the complete fixture outgoing queue, and restore the original peer, asset,
permission and technical-registration configuration. The incoming batch credits
only the replacement owner. Its attacker transfer is consumed without a mint.
Both native Done acknowledgements are journaled without appending outbound
messages. The synthetic outgoing records authorize no refund or balance credit.
The positive transfer mints one local XOR base unit. The native converter derives
its external amount using the checkpoint's unchanged denomination factor and
must round-trip exactly; at this checkpoint the factor is 10^38. A raw external
amount of 10^18 would convert to zero, while one whole local XOR would overflow
the sidechain's u128 amount. Neither denomination nor precision is patched for
the fixture.

Native preview derives the complete Incoming and Outgoing write sets, including
receipts and tombstones. The outer `fixtureExpectations` file binds those exact
effects and the final business state. Receipts contain the policy commitment, so
their hashes are deliberately kept outside the policy to avoid a commitment
cycle. Independent nonce, balance, issuance, acknowledgement and preservation
audits remain in the policy itself. Exact Wasm execution, independent block
import and fresh disk reconstruction must reproduce those effects and retain
them after runtime 138 activation.

The native preview runs only through the runtime's ignored unit-test exporter,
using the existing `cfg(test)` policy scope. The utility requires an absolute
`SORA_RECOVERY_FIXTURE_RUNTIME_TEST_BINARY` and the independently pinned
`SORA_RECOVERY_FIXTURE_RUNTIME_TEST_SHA256`; it starts one fixed exact test, never
Cargo or a shell. Each pass verifies the actual checkpoint state root at V0 and
binds its typed output to the full input hash. Public input, output and process
logs are retained with provenance. This mechanism adds no policy override to
node binaries or runtime Wasm, and does not weaken the production fail-closed
currency hooks when compiled policy is absent.

Neither native preview nor source generation claims a completed exact-artifact
rehearsal, GRANDPA proof, operator approval, external finality or deployable
release. All generated artifacts must retain their never-deploy labels.
