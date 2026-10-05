# Funded keepers and incoming relayers

Runtime activation requires the following operator preparation. The runtime does
not install keys, fund accounts, or contact operators automatically.

## Kensetsu and Apollo workers

1. Generate a separate sr25519 operational account. Do not reuse BABE, GRANDPA,
   ImOnline, or bridge proof keys. Both workers use the application key type
   `keep` (the four UTF-8 bytes `0x6b656570`).
2. Fund its native XOR balance from the operator's approved account. Estimate
   the normal transaction fee for `kensetsu.accrue`, `kensetsu.liquidate`, and
   `apolloPlatform.liquidate` using the candidate runtime and maintain a balance
   buffer for repeated failures and the configured worker submission limits.
   These maintenance calls pay fees on both success and failure.
3. Install the operational key only through the node's local authorized key
   management interface. The Substrate `author_insertKey` request has parameters
   `["keep", "<secret URI>", "0x<sr25519 public key>"]`. Never place its secret
   URI in shared logs, release evidence, or remote public RPC requests.
4. Confirm that the public key identifies the funded account and that the node
   runs offchain workers. A missing key logs a warning and sends no maintenance
   transaction. The signed transaction builder includes normal nonce, genesis,
   runtime versions, mortal era, weight, and fee extensions.
5. Before activation, rehearse a signed maintenance success and failure using
   the candidate runtime. Confirm a nonzero fee, correct sender/nonce, and no
   genuine unsigned maintenance admission. Monitor the operational balance and
   refill it through the existing approved funding process.

Multiple keeper keys on the same node are unnecessary: the workers select the
first available `keep` account. Use one funded key per operational lane. A shared
offchain lock and bounded 32-operation queue coordinate Kensetsu and Apollo
nonces. Pending operations are deduplicated and rebroadcast using exact signed
bytes once per block. Confirmed entries are removed; expired or version-changed
entries are re-signed at the same nonce, so later pending transactions have no
nonce gap. An operation that becomes stale while pending can subsequently fail
and pay a fee. Two independent nodes using the same account must coordinate
transaction nonces or use separate funded accounts.

## Generic and federated Substrate bridge protocol

Bridge peers and relayers do not need XOR. Existing bare peer-proof submissions
remain supported, and signed authenticated submissions are also free. The
commitment and proof formats do not change.

Admission verifies the authenticated remote proof, matching network/channel and
fresh protocol nonce before dispatch. Invalid, unsupported and replayed bare
requests are rejected in both transaction validation and pre-dispatch. Signed
requests receive zero-fee admission only when those same read-only checks pass.
Generic arbitrary signed calls do not receive the bridge exemption.

An accepted commitment consumes its protocol nonce even if a local application
fails. Successful batch items remain applied; failure reporting still identifies
failed credits. The authenticated delivery remains free, including an empty
remote batch or partial local failure, and cannot be replayed. Authenticated
status reports remain free when reporting a remote transfer failure.

Existing payload and batch bounds remain in place. Submission weights use a
conservative three-times bound for the additional admission checks until fresh
benchmarks are collected. Activation evidence must cover valid zero-XOR bare and
signed delivery, invalid proof, stale nonce, partial application, hidden mint
failure, empty batch, read-only validation and pre-dispatch rejection. Keeper
maintenance is a separate paid operation and still needs a funded `keep` key.
