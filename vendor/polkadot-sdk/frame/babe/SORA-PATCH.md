# SORA equivocation report fee policy

Upstream: `paritytech/polkadot-sdk`, tag `polkadot-stable2603-3`,
commit `e3737178ec726cffe506c907263aaaa417893fd0`,
source directory `substrate/frame/babe`. The Apache 2.0 license and
original file notices are retained. Workspace dependencies and lint settings
are made explicit for the SORA workspace, following the existing staking fork.

Local policy:

- Successful `report_equivocation` calls retain `Pays::Yes`.
- The existing `report_equivocation_unsigned` name, SCALE call index and proof
  arguments are retained as a compatibility alias. It now requires a signed
  origin, uses that signer as the reporter, and retains `Pays::Yes`.
- Unsigned transaction validation and pre-dispatch always reject with
  `InvalidTransaction::Call`, including local and in-block submissions.
- `submit_unsigned_equivocation_report` returns `None` without publishing
  evidence or making offchain host calls. Reports must be submitted as signed
  transactions through either report call.
- Proof validation, offence processing, duplicate rejection and report weights
  retain the upstream behavior.

Unit tests cover both signed call variants, retained successful and duplicate
report fees, rejection of every unsigned transaction source, unsigned origin
rejection, and the disabled publication helper.
