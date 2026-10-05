# Retired lending protocols and incoming relayers

SORA 4.8.12 configures Kensetsu and Apollo in repayment-only mode. The previous
funded-keeper rollout plan is superseded for these two protocols. Their offchain
workers return before scanning positions, accessing keys or submitting work.
Operators do not need to install or fund `keep` keys for this release.

## Existing-position exits

Kensetsu owners retain `repay_debt` and `close_cdp` to settle existing debt and
recover collateral. New CDPs, borrowing, additional collateral deposits,
standalone accrual, donations and liquidation are disabled at pallet dispatch,
including when calls are wrapped. Debt-free positions can close without
interest calculations or treasury mint permission. Repayment/closure checks
ownership before accounting, and failures roll back debt, tokens and collateral.
Existing debt continues accruing interest under its current terms. Repayment and
closure retain the existing fee calculation and treasury accounting, including
its settlement mint. No new borrower debt or borrower mint is permitted.

Apollo retains repayment, principal withdrawal and earned-reward claims. New
pools, lending deposits, borrowing, additional collateral and liquidation are
disabled. A last lender may withdraw exactly the available pool liquidity.
Principal/collateral exits preserve unpaid APOLLO rewards as separately
claimable records and do not require a funded reward pot. Existing borrowing
interest terms remain in force. Received interest is reserved in its original
asset and tracked in `DeferredProtocolInterest`, excluded from lending liquidity;
DEX buybacks cannot block repayment. Distribution of those reserves requires a
separately reviewed governance change.

All normal signed exit calls and rejected signed attempts pay transaction fees.
Bare maintenance calls remain rejected. Before activation, exercise existing
position exits and confirm new activity and automatic submissions are blocked.
The shared keeper helper remains generic library code, unused by these production
workers. BABE/GRANDPA signed reporting is a separate deployment requirement.

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
failure, empty batch, read-only validation and pre-dispatch rejection. Kensetsu and Apollo
maintenance is disabled; the bridge exception does not depend on keeper funding.
