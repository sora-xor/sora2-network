# SORA Nexus terminal conservation and paged activation checks

The compiler pins checkpoint issuance, explicit attacker burns, and each exact
post-edit issuance record. It includes every registered checkpoint asset and every
retained token issuance row, even when no substituted account owns that asset.
An absent native `ValueQuery` issuance record is canonical zero. An incoming asset
created only by later recovery work needs a reviewed baseline; an uncovered asset
blocks activation.

Incoming mint amounts are not taken from JSON, gross withdrawal amounts, or
caller-authored success flags. Inside the authenticated incoming transaction the
runtime reads native `Balances.TotalIssuance` or `Tokens.TotalIssuance` before and
after native execution. Its protected journal binds the exact baseline and final
record hashes, retains positive and negative supply deltas separately, and checks
continuity on every subsequent incoming operation. Negative deltas include native
extinction of previously prepared attacker claims. Custody unlocks and exclusions
that leave issuance unchanged contribute zero. Failed native calls roll the
journal back together with their nonce, receipts and balances.

Terminal equations use that native journal to extend their immutable baseline.
Every journaled asset must have one terminal issuance equation. Static storage
witnesses for a changed issuance record may follow the same exact native journal;
other static records retain exact hashes. The compiler refuses manually supplied
`authenticated_mints`. Existing public fixture builders no longer patch mint totals
or final issuance hashes by hand. The native journal does not authenticate external
finality or establish archive completeness; those remain independent release gates.

A raw issuance edit between incoming operations, or after the final incoming,
fails continuity or the terminal check. Forward repair preserves the journal and
cannot silently reset its baseline. A repair that genuinely changes such issuance
needs a separately reviewed adapter; this implementation fails closed instead of
assuming a new baseline.

Terminal checks larger than 128 fields are split deterministically into bounded
`TerminalAuditPage` actions (appended SCALE discriminant 11). The policy commits a
domain-separated ordered list of page hashes. Each page has at most 128 fields,
and at most 1,024 pages are accepted. A collateral equation is atomic: its retained
positions cannot be split across pages, so one equation exceeding 128 positions
still requires a dedicated bounded aggregation adapter.

The runtime accepts the first page only after all recovery work and ordinary
audits have completed and every incoming operation has its semantic audit. A
persisted policy/counter/manifest cursor rejects page skips and replay. Protected
per-field markers reject duplicate assets and positions across pages. Only the
last page writes the activation receipt and enters `Ready`. Forward repair starts
a new policy/counter cursor; earlier page receipts cannot advance it. Ordinary
economic execution remains disabled throughout paging.

Compiler reports retain `terminalEnvelope` as the last terminal action and add
`terminalPreludeEnvelopes` for earlier pages (empty for a single audit). Execute
that prelude after ordinary audits and code staging, immediately before the final
envelope. `recoveryWorkIndex` has the corresponding optional `terminalPrelude`
file-reference list. Every prelude shard belongs in the immutable release graph.
The transport, mixed-work compiler, maintenance rehearsal and post-state verifier
preserve and verify this exact sequence. Legacy single-envelope reports remain
readable. JSON alone is not execution evidence.
