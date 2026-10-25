#!/usr/bin/env python3
"""Reproduce the historical comparison in an isolated, non-production workspace."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--workspace", type=Path, required=True)
parser.add_argument("--target-dir", type=Path, required=True)
args = parser.parse_args()
package = Path(__file__).resolve().parents[1]
workspace = args.workspace.resolve()
repository = next(parent for parent in package.parents if (parent / ".git").exists())
if workspace == repository or repository in workspace.parents:
    parser.error("The historical comparison workspace must be outside the repository")
manifest = workspace / "Cargo.toml"
if manifest.exists() and tomllib.loads(manifest.read_text()).get("package", {}).get("name") != "sora2-libsecp-differential":
    parser.error("Refusing to overwrite an unrelated Cargo workspace")
workspace.mkdir(parents=True, exist_ok=True)
(workspace / "src").mkdir(exist_ok=True)
shutil.copyfile(Path(__file__).with_name("differential.rs"), workspace / "src/main.rs")
(workspace / "Cargo.toml").write_text(
    '[package]\nname="sora2-libsecp-differential"\nversion="0.1.0"\nedition="2021"\n'
    '[workspace]\n[dependencies]\n'
    'legacy={package="libsecp256k1",version="=0.7.2"}\n'
    f'modern={{package="sora-secp256k1-compat",path={json.dumps(str(package))}}}\n'
    'k256="=0.14.0"\nhex="=0.4.3"\n', encoding="utf-8"
)
env = dict(os.environ, CARGO_TARGET_DIR=str(args.target_dir.resolve()))
subprocess.run(["cargo", "generate-lockfile"], cwd=workspace, env=env, check=True)
subprocess.run(["cargo", "run", "--locked", "--release"], cwd=workspace, env=env, check=True)
digest = hashlib.sha256((workspace / "transcript.bin").read_bytes()).hexdigest()
expected = "f6a5fb45b886c56224b89648ae0b8fd5aacb595e49d02909d2dc4d3ab8516962"
if digest != expected:
    raise SystemExit(f"Historical transcript changed: {digest}")
if (workspace / "signatures.txt").read_bytes() != (package / "tests/fixtures/legacy-signatures.txt").read_bytes():
    raise SystemExit("Historical signature fixtures changed")
print(f"Historical transcript SHA-256: {digest}")
