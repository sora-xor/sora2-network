#!/usr/bin/env python3
"""Compile and deploy SCCP ETH contracts to mainnet.

Safety behavior:
- Dry-run unless --execute is provided.
- Mainnet execution requires --ack-mainnet.
- Private key must come from file (never raw CLI value).
"""

from __future__ import annotations

import argparse
import re
import shlex
import subprocess
import sys
from pathlib import Path

ACK_TOKEN = "I_UNDERSTAND_MAINNET_DEPLOY"
CHAIN_LABEL = "eth"
LOCAL_DOMAIN = "1"
EXPECTED_CHAIN_ID = "1"

SENSITIVE_FLAGS = {"--rpc-url"}


def redact_command(cmd: list[str]) -> str:
    parts: list[str] = []
    redact_next = False
    for token in cmd:
        if redact_next:
            parts.append("<redacted>")
            redact_next = False
            continue
        parts.append(token)
        if token in SENSITIVE_FLAGS:
            redact_next = True
    return shlex.join(parts)


def run(cmd: list[str], cwd: Path) -> None:
    print(f"+ {redact_command(cmd)}")
    subprocess.run(cmd, cwd=str(cwd), check=True)


def read_file_value(path: str) -> str:
    p = Path(path).expanduser().resolve()
    if not p.exists():
        raise SystemExit(f"File not found: {p}")
    return p.read_text(encoding="utf-8").strip()


def resolve_rpc_url(args: argparse.Namespace) -> str:
    if args.rpc_url:
        return args.rpc_url.strip()
    if args.rpc_url_file:
        v = read_file_value(args.rpc_url_file)
        if v:
            return v
    raise SystemExit(
        "Missing RPC URL: provide --rpc-url or --rpc-url-file"
    )


def resolve_private_key(args: argparse.Namespace) -> str:
    if not args.private_key_file:
        raise SystemExit("Missing private key: provide --private-key-file")
    v = read_file_value(args.private_key_file)

    if not re.fullmatch(r"0x[0-9a-fA-F]{64}", v):
        raise SystemExit("Private key must be 0x-prefixed 32-byte hex")
    return v


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Compile + deploy SCCP contracts to ETHEREUM mainnet")

    p.add_argument("--rpc-url", default=None, help="Mainnet JSON-RPC URL (avoid if URL contains secrets)")
    p.add_argument("--rpc-url-file", default=None, help="File containing RPC URL")

    p.add_argument("--private-key-file", default=None, help="File containing deployer private key")

    p.add_argument("--latest-beefy-block", required=True, help="Initial latest finalized BEEFY block")
    p.add_argument("--current-vset-id", required=True, help="Current validator-set id")
    p.add_argument("--current-vset-len", required=True, help="Current validator-set length")
    p.add_argument("--current-vset-root", required=True, help="Current validator-set root (bytes32)")
    p.add_argument("--next-vset-id", required=True, help="Next validator-set id")
    p.add_argument("--next-vset-len", required=True, help="Next validator-set length")
    p.add_argument("--next-vset-root", required=True, help="Next validator-set root (bytes32)")

    p.add_argument("--out", default=None, help="Output JSON path")
    p.add_argument("--state-file", default=None, help="Checkpoint state JSON path")
    p.add_argument("--resume", action="store_true", help="Resume from existing --state-file")

    p.add_argument("--skip-compile", action="store_true", help="Skip `npm run compile:deploy`")
    p.add_argument("--execute", action="store_true", help="Send real mainnet transactions")
    p.add_argument("--ack-mainnet", default=None, help=f"Must equal {ACK_TOKEN} with --execute")

    return p.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent

    rpc_url = resolve_rpc_url(args)
    resolve_private_key(args)

    if args.execute and args.ack_mainnet != ACK_TOKEN:
        print(
            f"Refusing mainnet deploy: pass --ack-mainnet {ACK_TOKEN}",
            file=sys.stderr,
        )
        return 2

    if not args.skip_compile:
        run(["npm", "run", "compile:deploy"], cwd=repo_root)

    cmd = [
        "node",
        "scripts/deploy_mainnet.mjs",
        "--chain-label",
        CHAIN_LABEL,
        "--rpc-url",
        rpc_url,
        "--private-key-file",
        str(Path(args.private_key_file).expanduser().resolve()),
        "--local-domain",
        LOCAL_DOMAIN,
        "--expected-chain-id",
        EXPECTED_CHAIN_ID,
        "--latest-beefy-block",
        str(args.latest_beefy_block),
        "--current-vset-id",
        str(args.current_vset_id),
        "--current-vset-len",
        str(args.current_vset_len),
        "--current-vset-root",
        str(args.current_vset_root),
        "--next-vset-id",
        str(args.next_vset_id),
        "--next-vset-len",
        str(args.next_vset_len),
        "--next-vset-root",
        str(args.next_vset_root),
    ]

    if args.out:
        cmd += ["--out", args.out]
    if args.state_file:
        cmd += ["--state-file", args.state_file]
    if args.resume:
        cmd += ["--resume"]

    if args.execute:
        cmd += ["--execute", "--ack-mainnet", args.ack_mainnet]

    run(cmd, cwd=repo_root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
