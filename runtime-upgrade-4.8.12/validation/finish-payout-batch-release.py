#!/usr/bin/env python3
"""Finish 4.8.12 evidence from freshly completed native/mainnet build logs.

Run only after freezing runtime sources and releasing the shared Cargo target.
Feature builds and Clippy run sequentially in that warm target. Public scripts
read chain state and encode unsigned calls; this script never submits a call.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
PACKAGE = HERE.parent
ROOT = PACKAGE.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--native-log", type=Path, required=True)
parser.add_argument("--format-log", type=Path, required=True)
parser.add_argument("--wasm-log", type=Path, required=True)
parser.add_argument("--native-command-file", type=Path, required=True)
parser.add_argument("--wasm-command-file", type=Path, required=True)
parser.add_argument("--target", type=Path, default=Path("/Users/takemiyamakoto/dev/.sora2-pr1366-target"))
parser.add_argument("--llvm", type=Path, default=Path("/opt/homebrew/opt/llvm@22"))
parser.add_argument("--work", type=Path, required=True)
parser.add_argument("--output", type=Path)
parser.add_argument("--resume-from", choices=["validation", "features", "clippy", "finalize"])
args = parser.parse_args()
work = args.work.resolve()
work.mkdir(parents=True, exist_ok=True)
target = args.target.resolve()
llvm = args.llvm.resolve()
assert all((llvm / name).exists() for name in ["bin/clang", "bin/llvm-ar", "bin/llvm-config", "lib"])
assert target.is_dir(), "Reuse the existing warm Cargo target"


def run(label, command, env=None, log=None):
    print(label, flush=True)
    if log:
        with Path(log).open("wb") as handle:
            subprocess.run(command, cwd=ROOT, env=env, stdout=handle, stderr=subprocess.STDOUT, check=True)
    else:
        subprocess.run(command, cwd=ROOT, env=env, check=True)


def script(name, *params, log=None):
    interpreter = "node" if name.endswith(".cjs") else sys.executable
    run(name, [interpreter, str(HERE / name), *map(str, params)], log=log)


frozen = json.loads((HERE / "source-provenance.json").read_text())
for name, expected in frozen["files"].items():
    assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected, "Source changed after build freeze: " + name
if args.resume_from is None:
    script("capture-source.py")
    captured = json.loads((HERE / "source-provenance.json").read_text())
    assert captured["sourceTreeAfterPatch"] == frozen["sourceTreeAfterPatch"], "Build freeze changed"
    wasm_output = target / "release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm"
    shutil.copyfile(wasm_output, PACKAGE / "framenode-runtime-4.8.12.compact.compressed.wasm")
    script("record-build-evidence.py", "--native-log", args.native_log,
           "--format-log", args.format_log, "--wasm-log", args.wasm_log,
           "--native-command", args.native_command_file.read_text().strip(),
           "--wasm-command", args.wasm_command_file.read_text().strip(), "--target", target)
else:
    # A failed later stage may have replaced the warm target with a feature
    # runtime. Never recopy it over the already recorded mainnet candidate.
    build = json.loads((HERE / "wasm-build.json").read_text())
    assert build["sourceTreeAfterPatch"] == frozen["sourceTreeAfterPatch"]
    assert hashlib.sha256((PACKAGE / "framenode-runtime-4.8.12.compact.compressed.wasm").read_bytes()).hexdigest() == build["candidateSha256"]


def at_or_before(stage):
    stages = ["validation", "features", "clippy", "finalize"]
    return args.resume_from is None or stages.index(args.resume_from) <= stages.index(stage)

# Baseline and bridge readiness must already have been recaptured. The live
# preflight below refuses packaging if that deployed code/metadata has changed.
if at_or_before("validation"):
    for name, logfile, params in [
        ("verify-wasm-compatibility.cjs", "wasm-compatibility.log", []),
        ("rehearse-equivocation-fees.cjs", "equivocation-fee-wasm-rehearsal.log", ["--synthetic-fixtures"]),
        ("rehearse-fee-policy.cjs", "fee-policy-wasm-rehearsal.log", ["--synthetic-fixtures"]),
        ("verify-fee-events.py", None, []),
        ("prepare-package.cjs", "prepare-package.log", []),
        ("prepare-governance-calls.cjs", "prepare-governance-calls.log", []),
        ("prepare-council-settings.py", None, []),
        ("verify-governance-calls.cjs", "governance-call-check.log", []),
    ]:
        script(name, *params, log=(HERE / logfile if logfile else None))

if at_or_before("features"):
    wasm_output = target / "release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm"
    build_env = {
        "CARGO_TARGET_DIR": str(target), "WASM_BUILD_WORKSPACE_HINT": str(ROOT),
        "WASM_BUILD_CARGO_ARGS": "--offline", "FORCE_WASM_BUILD": "4.8.12-batch-payout",
        "CC_wasm32_unknown_unknown": str(llvm / "bin/clang"),
        "AR_wasm32_unknown_unknown": str(llvm / "bin/llvm-ar"),
        "LIBCLANG_PATH": str(llvm / "lib"), "LIBRARY_PATH": str(llvm / "lib"),
        "LLVM_CONFIG_PATH": str(llvm / "bin/llvm-config"),
    }
    env = {**os.environ, **build_env}
    env.pop("SKIP_WASM_BUILD", None)
    feature_results = {}
    for profile, features in [
        ("stage", "build-wasm-binary,private-net,stage"),
        ("test", "build-wasm-binary,private-net,stage,wip,reduced-pswap-reward-periods"),
    ]:
        command = ["scripts/with_llvm_env.sh", "cargo", "build", "--release", "--offline", "--locked",
                   "-p", "framenode-runtime", "--features", features]
        log = work / (profile + "-build.log")
        run(profile + " runtime build", command, env=env, log=log)
        wasm = work / (profile + ".compact.compressed.wasm")
        shutil.copyfile(wasm_output, wasm)
        command_text = shlex.join(["env", "-u", "SKIP_WASM_BUILD",
                                  *(key + "=" + value for key, value in build_env.items()), *command])
        feature_results[profile] = {"wasm": wasm, "log": log, "command": command_text}
    script("refresh-chain-specs.py", ROOT,
           "--stage-wasm", feature_results["stage"]["wasm"], "--test-wasm", feature_results["test"]["wasm"],
           "--provenance", HERE / "source-provenance.json", "--output", HERE / "chain-spec-runtimes.json",
           "--stage-log", feature_results["stage"]["log"], "--stage-command", feature_results["stage"]["command"],
           "--test-log", feature_results["test"]["log"], "--test-command", feature_results["test"]["command"])

if at_or_before("clippy"):
    lint_env = {**os.environ, "CARGO_TARGET_DIR": str(target), "CARGO_NET_OFFLINE": "true",
                "LIBCLANG_PATH": str(llvm / "lib"), "LIBRARY_PATH": str(llvm / "lib"),
                "LLVM_CONFIG_PATH": str(llvm / "bin/llvm-config")}
    lint_command = ["scripts/with_llvm_env.sh", "./housekeeping/clippy.sh", "all"]
    lint_log = work / "clippy.log"
    run("Three-profile Clippy", lint_command, env=lint_env, log=lint_log)
    lint_evidence = shlex.join(["env", *(key + "=" + lint_env[key] for key in
                              ["CARGO_TARGET_DIR", "CARGO_NET_OFFLINE", "LIBCLANG_PATH", "LIBRARY_PATH", "LLVM_CONFIG_PATH"]),
                              *lint_command])
    script("record-clippy-evidence.py", "--log", lint_log, "--command", lint_evidence)

for name, expected in frozen["files"].items():
    assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected, "Source changed: " + name
script("finalize-docs.py", "--require-final-gates")
script("make-manifest.py")
script("verify-package.py", "--check-workspace-source")
script("make-zip.py", *([args.output] if args.output else []), "--check-workspace-source")
