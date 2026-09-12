# Watchdog source location audit — September 12, 2026

The three bridge fixes remain byte-for-byte identical to the six source files
recorded in `validation.json` for the 278 passing tests. This follow-up made no
runtime or test changes.

## Telegram evidence

Opened **SORA2 Production Watchdog** in Telegram again. The latest visible
message remains the September 11, 00:10 recovery notice. The visible September
10–11 sequence still repeats the same `2026-09-10 14:24:48.631` bridge warning
while its 60-minute count decreases and its 15-minute count is zero. It provides
no HTTP status, transport cause, provider identity, or scan-height evidence.

This is not proof of present bridge health or of the watchdog's implementation.

## Source candidates ruled out

- `soramitsu/infrastructure/scripts/sora2_healthchecker/app.py` implements
  sync-height HTTP health checks, not the observed Telegram RPC-error windows.
- `soramitsu/infrastructure/ansible/roles/monitoring-server/templates/telegram/default.tmpl.j2`
  formats Alertmanager FIRING/RESOLVED notifications with different fields.
- Infrastructure's OpenDistro/OpenSearch bridge monitor tasks use pending
  transaction thresholds over two minutes. The OpenDistro task was introduced
  in commit `3d1ecd0` on August 31, 2022; it is not the 15/60-minute log monitor.
- `soramitsu/sora2-deploy/roles/sora2/tasks/main.yml` and safe source path inventory
  contain no watchdog task. Its external-role submodule points to the repository
  below.
- `soramitsu/ansible-roles`, master at
  `c6f2edbde4d97fda74e0e30e5db0ee008db4f76c`, has no watchdog, monitoring, or Telegram
  role in the inspected source inventory.
- `soramitsu/sora2-bridge-alerts-bot`, master at
  `0c3eb4771f8c6fa79daa4fc839d1447b500a03f2`, is the source of the referenced
  `build-tools/bridge_alerts_bot` image. Its README, main loop, message formatting,
  and selected transaction checks monitor contract/transaction activity, not
  container log errors. Its notification text does not match this incident.

Watchdog commit searches in `sora2-deploy` and `infrastructure` returned no
matches. These bounded searches rule out the inspected candidates; they do not
prove that the source exists nowhere else.

A local watchdog-filename search under the development tree and Codex worktrees
returned only unrelated dependency files. The local Codex automation definitions
and script files had no matches for the watchdog title or RPC-error text.

A further search of accessible local task records for the watchdog title,
`bridge/RPC errors=`, `SORA2 alert`, and `SORA2 recovered` found no independent
implementation. Matches in records created on September 3 and 5 were September
12 task-list results referencing this investigation, not earlier watchdog code.
The remaining alert-text match was the original user report inherited by this
task's code-review agent. No historical commands were executed.

## Evidence still required

1. The actual watchdog source or repository path, to reproduce and correct its
   alert/recovery transitions in the implementation that sends these messages.
2. The HTTP status or transport-error detail immediately preceding the reported
   `sora2-framenode-3` warning, to distinguish timeout, rate limit, range rejection,
   and other fetch failures. The generic warning cannot identify which occurred.

Both details have been requested from the user. No substitute watchdog was
created. All follow-up work was local or read-only source/chat inspection; no
SSH, infrastructure operations, release builds, images, or deployment occurred.
