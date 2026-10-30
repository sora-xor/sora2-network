# SORA Nexus recovery — operator notice (unpublished draft)

This notice describes the proposed recovery release. No release publication,
operator upgrade, signing stop, service containment or restart is asserted here.
Only a complete, verified outer release manifest and the coordinated restart
notice may supply production commands, authority inventories and artifact hashes.
The source templates deliberately cannot start production recovery unbound.

The retained checkpoint is the complete post-state of block **28,191,817**,
hash `0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0`.
All later activity on the abandoned branch is reverted. Historical blocks and
finality evidence through the checkpoint must remain available.

The intended release is **node 4.8.13**, an immutable legacy **spec-134**
substitute, a pinned **spec-135** hold runtime, and final **spec-138 / transaction
134**. The runtime-137 council package is superseded. Runtime 138 must be
installed by the recovery executive after its committed work and invariant
checks; it is not an ordinary council upgrade on the abandoned chain.

## Before restart

1. Stop all signing on the abandoned branch before restart. The release derives
   `lastObservedAbandonedSlot` from authenticated public header ancestry and sets
   the first recovery slot strictly above it. No operator signing-stop attestation
   is required as a release input. Public headers do not prove private signing
   history; never run old and recovery nodes concurrently with the same keys.
   This release admits abandoned-head slot **298871044** and recovery slots
   starting at **298871045**. A later observed head blocks packaging until the
   runtime backstop and its tests are updated to match the new public evidence.
2. Preserve an immutable copy of the abandoned database and its evidence. Prepare
   a **separate** database ending at block **28,191,816**. Every participating
   restored node must import the checkpoint under the recovery release; the
   release snapshot already contains that import. Do not edit or delete
   historical consensus evidence in place.
3. Use the release's new chain-spec ID, database path and protocol/fork identity.
   Runtime overrides are forbidden. The spec-134 substitute must remain byte
   identical for the lifetime of this recovery chain, including future syncs.
4. Retain the checkpoint GRANDPA, BABE and liveness signing keys. The release
   derives their active, queued and pending unions from checkpoint state and
   remaps owner accounts using the trusted CSV. Do not generate replacement
   consensus keys or remove retained keys for this release. Private keys remain
   with their operators; no manual public-key approval inventory is required.
5. Verify that the release pins checkpoint client set **7956** and applies
   recovery set **7957** with the same ordered authority keys and weights to
   both ordinary import and warp sync.
   An arbitrary high-bit authority-set reset is not supported.
   The voter starts only after the verified checkpoint import and authority
   transition. Recovery permanently rejects GRANDPA reports for sets through
   7956 and BABE reports for slots through 298871044; recovery-chain offences
   remain reportable. Startup validation failures are logged as warnings.
6. Verify all manifest file hashes, policy/data commitments and exact-artifact
   rehearsal results. Check the account mapping, derived accounts, council,
   checkpoint-derived consensus records, finalized external heads and bridge
   reconciliation evidence against their authenticated public records. A candidate package marked incomplete
   does not authorize restart.

## Restart steps for validators

The release's operator kit (`operator_kit/`) does steps 2, 4, 5 and 6 below:
`sora-recovery unpack` checks and unpacks the snapshot, and `sora-recovery run`
starts the node with the release chain spec, your existing keystore and network
key, and the recovery feeder. It ships as `sora-nexus-recovery-28191817-kit.tar.gz`
for building and running from source, and as the Docker Hub image
`sora2/substrate:recovery-28191817` built from the kit's Dockerfile. Its README
has the commands for both; the steps below are what it does.

1. Stop every old node that holds validator keys, including backup and sentry
   nodes, and keep them stopped. Recovery slots and GRANDPA sets after 7956 are
   still slashable, so running an old node beside a recovery node with the same
   keys is a double-signing offence.
2. Do not reuse your usual database. The abandoned chain finalized blocks after
   the checkpoint, and neither `framenode revert` nor GRANDPA can undo finalized
   blocks. Node 4.8.13 also refuses any database whose checkpoint was imported by
   the old release. Use one of these instead:
   - The release snapshot. It holds the canonical chain through block
     **28,191,816** and checkpoint block **28,191,817**, imported by the release
     node, so it carries the recovery import record. It is valid only with the
     chain spec it was made with. Check its SHA-256 against the release manifest,
     then unpack it into your base path as the account that runs the node:
     `zstd -dc sora-nexus-recovery-28191817-db.tar.zst | tar -C <base-path> --no-same-owner -xf -`.
     This creates `<base-path>/chains/sora-nexus-recovery-28191817/db/full`.
   - Warp sync, starting from an empty database. This works only after the
     recovery chain has finalized blocks and at least 3 recovery peers serve it.
3. More than 2/3 of validators must start from the snapshot: at least **11 of
   the 15** checkpoint GRANDPA authorities. GRANDPA cannot finalize the first
   recovery blocks without them, and warp sync cannot begin before that finality
   exists. Coordinate who uses the snapshot before restart.
4. Keep your existing keystore and point the node at it. The recovery chain ID
   moves the default keystore from `<base-path>/chains/sora-substrate-main-net/keystore`
   to `<base-path>/chains/sora-nexus-recovery-28191817/keystore`, which is empty.
   Start the node with `--keystore-path <base-path>/chains/sora-substrate-main-net/keystore`,
   or copy that directory to the new location. Without its retained GRANDPA key
   the node refuses to start as a validator. Do not generate or rotate keys.
   The network key moves the same way, and a validator without one refuses to
   start (`NetworkKeyNotFound`). Pass
   `--node-key-file <base-path>/chains/sora-substrate-main-net/network/secret_ed25519`
   (or your usual `--node-key`/`--node-key-file`) to keep your peer ID, so the
   mainnet bootnode and reserved-peer addresses stay valid. Never use
   `--unsafe-force-node-key-generation`.
5. Start node 4.8.13 with the release chain spec, your usual base path and no
   runtime overrides. The snapshot is an archive RocksDB database: use
   `--state-pruning archive` or omit that flag, and do not select ParityDB.
   From the snapshot the GRANDPA voter starts at once with set 7957; after warp
   sync it starts once the checkpoint and set 7957 are imported. Any warning that
   begins `GRANDPA remains stopped before verified recovery` means the database
   or chain spec differs from the release.
6. Run the recovery feeder beside every validator until it reports `complete`.
   Block authors read the next envelope from `--recovery-inherent-dir`; the
   feeder writes the release's rehearsed envelopes there, one for each new best
   block, and signs or submits nothing. A validator without it authors
   maintenance ticks, which delay activation but approve no skipped work.

## During recovery

The maintenance executive accepts only timestamp/consensus handling, committed
recovery work and the quorum-authorized escape path. Economic sessions/eras,
ordinary transactions and economic hooks remain frozen. Node offchain workers
remain suppressed while historical state is imported and while maintenance runs.

Recovery envelopes are selected by exact parent hash. A missing envelope produces
a maintenance tick; it does not approve skipped work. Restart resumes the stored
cursor. A committed invariant failure requires investigation or the pinned
recovered council's **strict majority** escape to hold 135. Signatures must bind
the complete action, checkpoint, policy, phase, cursor, code identities and
persistent operation counter. Versions 136/137 are reserved for reviewed forward
repairs. Never replace the immutable 134 substitute to repair finalized history.

After the terminal audit and all pinned final-code chunks are present, activation
of runtime 138 is automatic. The deferred session transition receives the
existing one-session liveness grace. Kensetsu's fee clocks exclude maintenance
downtime once, while pre-checkpoint debt remains.

## After activation

The bridge's on-chain outbound and worker halt remains active. Reopening is a
separate coordinated action after external nonces and withdrawal authorizations
are reconciled. Previously issued external authorizations may still be redeemable;
the local fork cannot invalidate them. Do not advertise cancellation as proof of
external non-redemption, or manually refund quarantined liabilities.

### Bridge transfers after the checkpoint

Bridge peers signed these withdrawals on the abandoned branch after the
checkpoint. The fork restores their SORA-side debits, and this release debits no
sender again: each keeps its checkpoint balance, or its trusted-CSV replacement
account receives it. `cnSBot6vTtpckdo7HQhjk2hiAAKfDJX34DZsyLVs7B5Y2Gv6T` is not
the attacker, and its funds are not reverted. The bridge reserve bears the paid
amounts. The two signed, unclaimed withdrawals stay redeemable on Ethereum
(bridge contract `0x313416870a4da6f12505a550b67bb73c8e21d5d3`).

| SORA sender | Ethereum recipient | Amount | On Ethereum |
|---|---|---|---|
| `cnSBot6vTtpckdo7HQhjk2hiAAKfDJX34DZsyLVs7B5Y2Gv6T` | `0x94b3c5fc67b0c63befcc74d7afcd333baa697b2a` | 8,750,000.0000 PSWAP, 7,507.0626 DAI | paid |
| `cnSBot6vTtpckdo7HQhjk2hiAAKfDJX34DZsyLVs7B5Y2Gv6T` | `0x94b3c5fc67b0c63befcc74d7afcd333baa697b2a` | 43,000.0000 VAL | signed, unclaimed |
| `cnSgmhqLV3sXSz72o4JjMZeBLX39Y6vN4vwjdaXAmULsGQHZe` | `0xaf6fd7a11087bfbaddc5c4e1ab14ab0c592af63f` (attacker) | 148.6001 DAI, 8.0235 XOR | paid |
| `cnW7wojJ6GgJz5VtqjAMT5CUmkuKXfT4MDsPzyRUWF2qyc8wV` | `0x95d64b2a397c4de7027e02fb0389965a68b8b376` | 176.7523 of asset `0x00513be65493a7fc3e2128d4230061a530acf40478a4affa20bbba27a310673e` | paid |
| `cnTBZzshYXajet389PSAHKsyZ72ASL7XZhNCiudFJJ24okTDD` (in the CSV) | `0xaf6fd7a11087bfbaddc5c4e1ab14ab0c592af63f` (attacker) | 13,992,342.6464 PSWAP, 2,029.9013 DAI | paid |
| `cnTBZzshYXajet389PSAHKsyZ72ASL7XZhNCiudFJJ24okTDD` (in the CSV) | `0xaf6fd7a11087bfbaddc5c4e1ab14ab0c592af63f` (attacker) | 20,601.8127 VAL | signed, unclaimed |
| `cnTYBcwoqVSiruign6ZsJbLvFnH4iRiNFm2tdA6idWZwAEupM` (in the CSV) | `0xaf6fd7a11087bfbaddc5c4e1ab14ab0c592af63f` (attacker) | 18,868,090.1949 PSWAP | paid |

The fork also drops the one incoming deposit credited on the abandoned branch:
11.6148 XOR to `cnSBot6vTtpckdo7HQhjk2hiAAKfDJX34DZsyLVs7B5Y2Gv6T` from Ethereum
transaction `0xdbd364b227386e144e17a652628c1813d6087c11f1428f61e95116193de341a1`.
The recovery chain has no request for that transaction. Once the bridge reopens,
credit it from that transaction through the normal incoming path, not manually.

Indexers and wallets must use the recovery fork anchor and executing-code
identity. Spec version alone cannot distinguish the deployed 134 runtime from
the immutable 134 substitute. Confirm TS/SS entitlement queries use the original
Burn-event identity and the effective replacement owner. Linked SORA Nexus XOR
claims for replaced accounts remain held until replacement-key recipient
authorization is verified.

Frontend and off-chain Soratopia points recovery are deferred until after the
node/runtime update. Existing evidence and historical points records must remain
available for that separate work; this release does not claim that the rewards
service or account queries have already been migrated.
