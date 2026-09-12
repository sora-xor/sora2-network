# Validation of the completed source fixes

The frontend patch targets Apps commit
`05aa55e844c492d516dba3c4b0bd1b9311f9aeb2` and changes 21 files.

SHA-256: `8de5d05c6a0374e6213f48f59a888c6feff30ce8af18d976cd23bd5f0f41e987`.

| Check | Result |
| --- | --- |
| Runtime payout/fee implementation tests | 62 passed, 1 pre-existing ignored test |
| Runtime fee integration tests | 52 passed, 1 pre-existing ignored test |
| Runtime with `wip`, payout and fee tests | 119 passed, 2 pre-existing ignored tests |
| Complete vendored staking pallet suite | 213 passed |
| Complete xor-fee pallet suite | 49 passed |
| WebAssembly target check, default and `wip` | Passed both |
| Focused frontend regression runner | 32 passing runner entries, including suites; zero failures |
| Full Apps `yarn build:code` | Passed, including all TypeScript projects and 188 type-bundle runner entries |
| ESLint | Passed on all 21 changed frontend files |
| Frontend patch application | Passed against a clean index of the pinned upstream commit |
| Vendored staking patch reconstruction | Exactly reproduces all six changed upstream files |
| Whitespace checks | Passed for the task's runtime/pallet files and frontend patch |

Exact commands and source fingerprints are in
[validation-results.json](validation-results.json). The runtime commands reuse
the checkout's existing `target` directory. Native tests skip generation of a
release Wasm blob; the additional `wasm32-unknown-unknown` checks compile the
actual no-std runtime for both feature sets. LLVM 21 provides the C compiler
for the Wasm target on this Mac.

The frontend install used the checked-in Yarn 4.6.0 release and immutable
lockfile. Optional dependency install scripts were disabled; the full code
build completed successfully. A browser wallet signing smoke test and an
optimized release runtime artifact were not produced in this code-fix task.

## Regression coverage

Payout tests execute real staking dispatch rather than simulating successful
post-dispatch hooks. They verify both payout entry points, paged and legacy
exposure, self-only validators, utility batch variants, multisig, exact-once
VAL payment, no extra XOR issuance, claim rollback and retry after a failed
mint, rollback of failed `batch_all`, and retained VAL execution weight when
native reward is zero.

The fee tests exercise actual withdrawal and settlement, native issuance
invariants, full refunds, referral rounding and legacy bucket initialization,
fee-token remint supply invariants, and routed buyback weight on success and
failure.

Frontend tests cover every page, own stake, commission and page rounding,
legacy fallback, mixed claimed validators, reactive claim updates in both stash
and Own validators modes, zero-own-stake validators with pending nominator
payouts, direct-call ordering/limits, and proxy/multisig signer guards needed by
deployed runtime 130.

## Mainnet evidence

[final-mainnet-validation.json](final-mainnet-validation.json) records the final
read-only check at finalized block
`0xe994c10c47f8295bc0ed657ae270fdd4c5577bf039f62e64e8546a11e60253d6`,
runtime 130, active era 7636. Both staking payout extrinsics remain exposed.
For the five completed eras checked, standard rewards were zero and the patched
derive matched the separate nonzero VAL storage. Staking account discovery,
stash rewards, and the new Own validators derive all completed successfully.
No transaction was signed or submitted.

The earlier live exposure snapshot in `bug-hunt/frontend-live-scope.json` had
one page for each of its 25 validators. Multi-page failures and repairs are
proven by controlled fixtures, not claimed as observed losses in that snapshot.
