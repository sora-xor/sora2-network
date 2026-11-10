#!/usr/bin/env python3
"""Compare maintained metering with the retired historical instrumenter in isolation."""
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
parser.add_argument("--runtime-wasm", type=Path)
args = parser.parse_args()
verification = Path(__file__).resolve().parent
package = verification.parent
workspace = args.workspace.resolve()
repository = next(parent for parent in package.parents if (parent / ".git").exists())
if workspace == repository or repository in workspace.parents:
    parser.error("The historical-reference workspace must be outside the repository")
manifest = workspace / "Cargo.toml"
if manifest.exists() and tomllib.loads(manifest.read_text()).get("package", {}).get("name") != "sora2-stack-metering-probe":
    parser.error("Refusing to overwrite an unrelated Cargo workspace")
workspace.mkdir(parents=True, exist_ok=True)
(workspace / "src").mkdir(exist_ok=True)
shutil.copyfile(verification / "stack_metering_reference.rs", workspace / "src/main.rs")
(workspace / "src/lib.rs").write_text(
    f'#[path={json.dumps(str(package / "src/runtime_blob/stack_metering.rs"))}]\n'
    'mod stack_metering;\n'
    'pub fn inject(raw:&[u8],limit:u32)->Result<Vec<u8>,String>{stack_metering::inject(raw,limit)}\n',
    encoding="utf-8",
)
(workspace / "Cargo.toml").write_text(
    '[package]\nname="sora2-stack-metering-probe"\nversion="0.1.0"\nedition="2021"\n'
    '[workspace]\n[dependencies]\n'
    'wasmparser={version="=0.262.0",features=["validate"]}\n'
    'wasm-encoder={version="=0.262.0",features=["wasmparser"]}\n'
    'wasm-instrument="=0.4.0"\nwat="=1.252.0"\n', encoding="utf-8",
)
env = dict(os.environ, CARGO_TARGET_DIR=str(args.target_dir.resolve()))
def run(*command):
    subprocess.run(command, cwd=workspace, env=env, check=True)
run("cargo", "generate-lockfile")
run("cargo", "test", "--locked", "--lib")
run("cargo", "run", "--locked")
run("node", str(verification / "execute_stack_metering.mjs"))
digest = hashlib.sha256((workspace / "execution-transcript.json").read_bytes()).hexdigest()
if digest != "4bb12f973655d147b992222b48a2aa118c8cb3af05e8b4c11709993e629a27f7":
    raise SystemExit(f"Historical execution transcript changed: {digest}")
print(f"Historical execution transcript SHA-256: {digest}")
if args.runtime_wasm:
    run("cargo", "run", "--locked", "--", str(args.runtime_wasm.resolve()))
    for name in ["runtime.old.wasm", "runtime.new.wasm"]:
        digest = hashlib.sha256((workspace / name).read_bytes()).hexdigest()
        print(f"{name} SHA-256: {digest}")
