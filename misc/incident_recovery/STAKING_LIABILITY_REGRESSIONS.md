# SORA Nexus recovery staking liability regressions

`runtime/src/incident_recovery_staking_tests.rs` exercises the native runtime's
financial handlers after a guarded account replacement. The fixture uses public,
synthetic identities. It creates a real historical session trie and ownership
proof, applies exact typed storage edits through `common::recovery::storage`, and
runs the sync-to-async staking migration. It never substitutes a test slashing or
reward calculation for the runtime implementation.

The three focused tests passed on the current source:

- `authenticated_historical_offence_debits_replacement_ledgers_without_rewriting_proof_root`
  authenticates the retained historical proof, observes the recovered ownership
  adapter return the replacement validator and nominator identities, and executes
  the native offence and deferred-slash paths. The replacement ledgers pay their
  penalties, the unaffected nominator retains its original liability, and the
  reporter reward goes to the replacement reporter. Historical root bytes stay
  identical.
- `remapped_historical_maxima_reject_repeat_and_only_charge_higher_native_increment`
  first generates authentic slash maxima, spans and paid-out amounts through the
  native handler. Moving those records preserves duplicate-report protection:
  the same fraction changes neither balances nor issuance, a higher fraction
  charges only its native increment, and repeating that higher fraction again
  pays and charges zero. The final financial boundary also canonicalizes an
  already-authenticated old identity idempotently.
- `remapped_claimed_pages_and_legacy_claim_markers_cannot_mint_rewards_again`
  creates a successful native VAL payout before replacement. After moving the
  page-claim and legacy ledger-claim state, both payout entry points reject
  already claimed rewards. A genuinely unclaimed era still pays exactly once.

These tests establish native handler behavior with deliberately small fixtures.
The typed fixture edits are not a substitute for the offline compiler's complete
checkpoint inventory, final CSV ownership proofs, actual operator inventory or
exact-artifact maintenance and activation rehearsals. They do not authorize a
release or rewrite historical headers, roots or proofs.
