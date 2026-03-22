#!/usr/bin/env python3
"""Compile and deploy SCCP TON contracts to mainnet.

Safety behavior:
- Dry-run unless --execute is provided.
- Mainnet execution requires --ack-mainnet.
- Mnemonic must come from file (never raw CLI value).
- Deployment is not complete until the governor-controlled verifier bootstrap is also done.
"""

from __future__ import annotations

import argparse
import shlex
import subprocess
import sys
from pathlib import Path

ACK_TOKEN = "I_UNDERSTAND_MAINNET_DEPLOY"
DEFAULT_ENDPOINT = "https://mainnet-v4.tonhubapi.com"

SENSITIVE_FLAGS = {"--endpoint"}


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


def resolve_path_arg(path: str | None) -> str | None:
    if path is None:
        return None
    return str(Path(path).expanduser().resolve())


def resolve_endpoint(args: argparse.Namespace) -> str:
    if args.endpoint:
        return args.endpoint.strip()
    if args.endpoint_file:
        v = read_file_value(args.endpoint_file)
        if v:
            return v
    return DEFAULT_ENDPOINT


def resolve_mnemonic(args: argparse.Namespace) -> str:
    if not args.mnemonic_file:
        raise SystemExit("Missing mnemonic: provide --mnemonic-file")
    mnemonic = read_file_value(args.mnemonic_file)

    if len([w for w in mnemonic.split() if w]) < 12:
        raise SystemExit("Mnemonic must have at least 12 words")
    return mnemonic


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Compile + deploy SCCP TON contracts to mainnet")

    p.add_argument("--endpoint", default=None, help="TON mainnet API v4 endpoint (avoid if URL includes secrets)")
    p.add_argument("--endpoint-file", default=None, help="File containing endpoint URL")

    p.add_argument("--mnemonic-file", default=None, help="File containing deployer mnemonic words")
    p.add_argument(
        "--governor-mnemonic-file",
        default=None,
        help="Optional file containing the configured governor wallet mnemonic for auto-sending the one-time SccpSetVerifier bind",
    )

    p.add_argument(
        "--governor",
        default=None,
        help="Governor TON address stored in contract init data and authorized for one-time bootstrap actions",
    )
    p.add_argument(
        "--sora-asset-id",
        default=None,
        help="32-byte SORA asset id hex",
    )
    p.add_argument("--metadata-uri", default="", help="Optional token metadata URI stored in master")
    p.add_argument("--master-value", default="0.25", help="TON value for master deployment message")
    p.add_argument("--verifier-value", default="0.45", help="TON value for verifier deployment message")
    p.add_argument(
        "--bind-verifier-value",
        default="0.05",
        help="TON value for the optional post-deploy SccpSetVerifier message",
    )
    p.add_argument(
        "--initialize-verifier-value",
        default="0.05",
        help="TON value for the optional post-deploy SccpVerifierInitialize message",
    )
    p.add_argument("--latest-beefy-block", default=None, help="Initial finalized SORA BEEFY block")
    p.add_argument("--current-validator-set-id", default=None)
    p.add_argument("--current-validator-set-len", default=None)
    p.add_argument("--current-validator-set-root", default=None, help="32-byte hex root")
    p.add_argument("--next-validator-set-id", default=None)
    p.add_argument("--next-validator-set-len", default=None)
    p.add_argument("--next-validator-set-root", default=None, help="32-byte hex root")
    p.add_argument("--out", default=None, help="Output JSON path")
    p.add_argument("--state-file", default=None, help="Checkpoint state JSON path")
    p.add_argument("--resume", action="store_true", help="Resume from existing --state-file")

    p.add_argument("--skip-build", action="store_true", help="Skip `npm run build`")
    p.add_argument("--execute", action="store_true", help="Send real mainnet transactions")
    p.add_argument("--ack-mainnet", default=None, help=f"Must equal {ACK_TOKEN} with --execute")

    return p.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent

    endpoint = resolve_endpoint(args)
    resolve_mnemonic(args)

    if not args.governor:
        print("Missing --governor", file=sys.stderr)
        return 2
    if not args.sora_asset_id:
        print("Missing --sora-asset-id", file=sys.stderr)
        return 2

    if args.execute and args.ack_mainnet != ACK_TOKEN:
        print(
            f"Refusing mainnet deploy: pass --ack-mainnet {ACK_TOKEN}",
            file=sys.stderr,
        )
        return 2

    if not args.skip_build:
        run(["npm", "run", "build"], cwd=repo_root)

    cmd = [
        "node",
        "scripts/deploy_mainnet.mjs",
        "--endpoint",
        endpoint,
        "--mnemonic-file",
        str(Path(args.mnemonic_file).expanduser().resolve()),
        "--governor",
        args.governor,
        "--sora-asset-id",
        args.sora_asset_id,
        "--metadata-uri",
        args.metadata_uri,
        "--master-value",
        args.master_value,
        "--verifier-value",
        args.verifier_value,
        "--bind-verifier-value",
        args.bind_verifier_value,
        "--initialize-verifier-value",
        args.initialize_verifier_value,
    ]

    if args.latest_beefy_block is not None:
        cmd += ["--latest-beefy-block", args.latest_beefy_block]
    if args.current_validator_set_id is not None:
        cmd += ["--current-validator-set-id", args.current_validator_set_id]
    if args.current_validator_set_len is not None:
        cmd += ["--current-validator-set-len", args.current_validator_set_len]
    if args.current_validator_set_root is not None:
        cmd += ["--current-validator-set-root", args.current_validator_set_root]
    if args.next_validator_set_id is not None:
        cmd += ["--next-validator-set-id", args.next_validator_set_id]
    if args.next_validator_set_len is not None:
        cmd += ["--next-validator-set-len", args.next_validator_set_len]
    if args.next_validator_set_root is not None:
        cmd += ["--next-validator-set-root", args.next_validator_set_root]

    if args.governor_mnemonic_file:
        cmd += ["--governor-mnemonic-file", str(Path(args.governor_mnemonic_file).expanduser().resolve())]

    if args.out:
        cmd += ["--out", resolve_path_arg(args.out)]
    if args.state_file:
        cmd += ["--state-file", resolve_path_arg(args.state_file)]
    if args.resume:
        cmd += ["--resume"]
    if args.execute:
        cmd += ["--execute", "--ack-mainnet", args.ack_mainnet]

    run(cmd, cwd=repo_root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
