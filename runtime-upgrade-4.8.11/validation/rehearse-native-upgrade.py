#!/usr/bin/env python3
"""Replay the runtime upgrade locally from pinned public state, failing closed."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--endpoint", choices=["wss://mof2.sora.org", "wss://ws.mof.sora.org"],
                    default="wss://mof2.sora.org")
parser.add_argument("--snapshot", type=Path, default=Path(tempfile.gettempdir()) /
                    "sora-staking-4811-mainnet-27834097.snap")
args = parser.parse_args()
scope = json.loads((HERE / "native-remote-scope.json").read_text())
assert scope["environment"]["REQUIRE_REMOTE"] == "1"
env = os.environ.copy()
env.update(scope["environment"])
env.update(SKIP_WASM_BUILD="1", REMOTE_RPC_URL=args.endpoint, SNAP=str(args.snapshot))
command = ["scripts/with_llvm_env.sh", "cargo", "test", "--release", "--offline", "--locked",
           "-p", "framenode-runtime", "--features", "try-runtime", "--lib",
           "remote_try_runtime_upgrade_rehearsal", "--", "--exact", "--nocapture"]
log = HERE / "native-remote-rehearsal.log"
with log.open("w") as output:
    result = subprocess.run(command, cwd=ROOT, env=env, stdout=output,
                            stderr=subprocess.STDOUT, check=False)
text = log.read_text()
passed = result.returncode == 0 and "test result: ok. 1 passed" in text and \
    "Runtime upgrade rehearsal replay:" in text and scope["sourceBlockHash"] in text
report = {
    "status": "passed" if passed else "failed",
    "sourceCommit": scope["sourceCommit"],
    "sourceBlockNumber": scope["sourceBlockNumber"],
    "sourceBlockHash": scope["sourceBlockHash"],
    "endpoint": args.endpoint,
    "snapshot": str(args.snapshot),
    "stateMode": "OfflineOrElseOnline; snapshot pinned block hash is checked by the test",
    "observedStateInput": "Live public RPC scrape" if "Scraping keys..." in text else "Offline snapshot",
    "scope": "native-remote-scope.json",
    "command": command,
    "exitCode": result.returncode,
    "log": log.name,
    "logSha256": hashlib.sha256(log.read_bytes()).hexdigest(),
    "submittedTransactions": 0,
    "limitations": scope["limitations"][1:],
}
(HERE / "native-remote-rehearsal.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report))
raise SystemExit(0 if passed else 1)
