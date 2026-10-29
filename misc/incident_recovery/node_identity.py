#!/usr/bin/env python3
"""Fixed offline node-version and prepared-template check; never node startup.

The caller must approve the native executable separately. Hashes identify bytes;
neither a CLI version string nor a successful offline command authenticates a
build. No input selects a command, script, environment, provider or endpoint.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import time

SCHEMA = "sora-nexus-node-offline-identity-v1"
CHECKPOINT_HASH = "0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0"
ROLES = {"nodeBinary": 2 * 1024**3, "chainSpec": 256 * 1024**2,
         "recoveryConfig": 2 * 1024**2, "bootstrapWasm": 50 * 1024**2}
COPY_NAMES = {"nodeBinary": "framenode", "chainSpec": "candidate.json",
              "recoveryConfig": "recovery-config.json", "bootstrapWasm": "bootstrap.wasm"}
MAX_LOG = 64 * 1024
# Offline preparation parses the full chain specification and validates every Wasm substitute.
# Keep finite limits generous enough for the shared build host under sustained load.
VERSION_TIMEOUT_SECONDS = 900
PREPARE_TIMEOUT_SECONDS = 3600
MAX_WASM_BYTES = 50 * 1024**2
SDK_ZSTD_PREFIX = bytes([82, 188, 83, 118, 70, 219, 142, 5])
CONFIG_FIELDS = {"schemaVersion", "checkpointHash", "policyCommitment", "policyScale", "bootstrapCodeHash",
                 "finalCodeHash", "holdCodeHash", "evidenceCommitment", "previousGrandpaSetId",
                 "previousGrandpaAuthorities", "grandpaAuthorities",
                 "approvedGrandpaKeys", "lastObservedAbandonedSlot", "firstRecoverySlot"}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def exact(value, names):
    need(isinstance(value, dict) and set(value) == set(names), "unknown or missing fields")


def strict_json(data):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, "duplicate JSON field")
            result[key] = value
        return result

    def finite(value):
        parsed = float(value)
        need(math.isfinite(parsed), "nonfinite JSON number")
        return parsed

    return json.loads(data, object_pairs_hook=pairs, parse_float=finite,
                      parse_constant=lambda _: need(False, "nonfinite JSON number"))


def physical(path, directory=False):
    path = Path(path).absolute()
    need(not any(c in str(path) for c in "\\\n\r\x00")
         and str(path) == str(PurePosixPath(path)) and ".." not in path.parts,
         "noncanonical physical path")
    current = Path(path.anchor)
    for part in path.parts[1:]:
        need(not any(word in part.lower() for word in ("sydneycreds",)), "credential path forbidden")
        need(part.lower() not in (".env", "keystore"), "credential path forbidden")
        current /= part
        info = current.lstat()
        need(not stat.S_ISLNK(info.st_mode), "symlink path forbidden")
    info = path.lstat()
    if directory:
        need(stat.S_ISDIR(info.st_mode), "directory required")
    else:
        need(stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
             "unaliased regular file required")
    return path, info


def relative(root, name):
    need(isinstance(name, str) and name and not any(c in name for c in "\\\r\n\x00"),
         "invalid relative path")
    parsed = PurePosixPath(name)
    need(not parsed.is_absolute() and str(parsed) == name and ".." not in parsed.parts,
         "noncanonical relative path")
    return physical(root / name)[0]


def file_pin(path, maximum):
    path, info = physical(path)
    need(0 < info.st_size <= maximum, "artifact size bound")
    h = hashlib.sha256()
    length = 0
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024**2), b""):
            length += len(block)
            need(length <= maximum, "artifact read bound")
            h.update(block)
    need(length == info.st_size, "artifact changed while reading")
    return {"sha256": h.hexdigest(), "bytes": length}


def write_json(path, value):
    with Path(path).open("x", encoding="utf-8") as stream:
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    descriptor = os.open(Path(path).parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def native_format(path):
    with path.open("rb") as stream:
        magic = stream.read(4)
    need(magic == b"\x7fELF" or magic in (b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf",
         b"\xce\xfa\xed\xfe", b"\xfe\xed\xfa\xce", b"\xca\xfe\xba\xbe",
         b"\xbe\xba\xfe\xca", b"\xca\xfe\xba\xbf", b"\xbf\xba\xfe\xca"),
         "nodeBinary must be a native ELF or Mach-O executable")


def read_bounded(path, maximum):
    _, info = physical(path)
    need(info.st_size <= maximum, "file read bound")
    with path.open("rb") as stream:
        data = stream.read(maximum + 1)
    need(len(data) <= maximum, "file read bound")
    return data


def run_fixed(argv, output, label, environment, timeout):
    """Only verify() constructs argv; no command text is accepted from evidence."""
    stdout_path, stderr_path = output / f"{label}.stdout", output / f"{label}.stderr"
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    deadline = time.monotonic() + timeout
    failure = None
    launch = {"argv": argv, "cwd": str(output), "environment": environment,
              "startedAt": started, "timeoutSeconds": timeout,
              "stdoutFile": stdout_path.name, "stderrFile": stderr_path.name,
              "completionObserved": False, "productionReady": False}
    launch_path = output / f"{label}.launch.json"
    write_json(launch_path, launch)
    started_path = output / f"{label}.started.json"
    with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
        process = subprocess.Popen(argv, cwd=output, env=environment, stdin=subprocess.DEVNULL,
                                   stdout=stdout, stderr=stderr, shell=False, start_new_session=True)
        try:
            write_json(started_path, {**launch, "pid": process.pid,
                       "launchReceiptSha256": file_pin(launch_path, MAX_LOG)["sha256"]})
            while process.poll() is None:
                if stdout_path.stat().st_size > MAX_LOG or stderr_path.stat().st_size > MAX_LOG:
                    failure = "command output bound"
                    break
                if time.monotonic() > deadline:
                    failure = "command timeout"
                    break
                time.sleep(0.02)
        finally:
            if process.poll() is None:
                # Kill the fixed command's process group, including accidental children.
                import signal
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            code = process.wait()
    log_pins = {}
    for name, path in (("stdout", stdout_path), ("stderr", stderr_path)):
        # Preserve failed-command logs, but never read an oversized log into memory.
        length = path.stat().st_size
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        log_pins[name] = {"file": path.name, "sha256": digest, "bytes": length}
        if length > MAX_LOG:
            failure = "command output bound"
    receipt = {"argv": argv, "cwd": str(output), "environment": environment,
               "pid": process.pid, "startedAt": started,
               "finishedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "exitCode": code, "failure": failure,
               "launchReceipt": {"file": launch_path.name, **file_pin(launch_path, MAX_LOG)},
               "startedReceipt": {"file": started_path.name, **file_pin(started_path, MAX_LOG)},
               **log_pins}
    write_json(output / f"{label}.process.json", receipt)
    need(failure is None, failure)
    need(code == 0, f"{label} command failed with exit code {code}")
    return receipt, read_bounded(stdout_path, MAX_LOG)


def authority_key(value):
    """Canonical typed JSON emitted by the fixed prepare command (SDK prefix42)."""
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    need(isinstance(value, str) and 1 <= len(value) <= 50
         and all(char in alphabet for char in value), "GRANDPA key must use canonical SS58 prefix42")
    number = 0
    for char in value:
        number = number * 58 + alphabet.index(char)
    data = b"\0" * (len(value) - len(value.lstrip("1"))) + number.to_bytes((number.bit_length() + 7) // 8, "big")
    need(len(data) == 35 and data[0] == 42, "GRANDPA key must use canonical SS58 prefix42")
    need(hashlib.blake2b(b"SS58PRE" + data[:-2], digest_size=64).digest()[:2] == data[-2:],
         "GRANDPA SS58 checksum mismatch")
    return data[1:-2]


def authority_json(config):
    # This mirrors serialization for this fixed offline command only; its actual
    # typed parser still validates inventories, weights and recovery semantics.
    for name in ("previousGrandpaAuthorities", "grandpaAuthorities"):
        rows = config[name]
        need(isinstance(rows, list) and 0 < len(rows) <= 1000, "bounded GRANDPA authority list required")
        for row in rows:
            need(isinstance(row, list) and len(row) == 2 and type(row[1]) is int
                 and 0 < row[1] < 2**64, "GRANDPA authority key/weight pair required")
            authority_key(row[0])
    for name, maximum in (("approvedGrandpaKeys", 3000),):
        keys = config[name]
        need(isinstance(keys, list) and 0 < len(keys) <= maximum, "bounded GRANDPA key inventory required")
        for key in keys:
            authority_key(key)


def validate_bootstrap(bootstrap):
    """Check bounded raw or single-frame SDK-compressed Wasm without rewriting it."""
    need(isinstance(bootstrap, bytes) and 0 < len(bootstrap) <= MAX_WASM_BYTES,
         "bootstrap Wasm size bound")
    decoded = bootstrap
    if bootstrap.startswith(SDK_ZSTD_PREFIX):
        from compression import zstd
        try:
            # Limit both output and decoder window allocation. SDK level22
            # artifacts request a 128MiB window even for code below 50MiB.
            decoder = zstd.ZstdDecompressor(options={zstd.DecompressionParameter.window_log_max: 27})
            decoded = decoder.decompress(bootstrap[len(SDK_ZSTD_PREFIX):], max_length=MAX_WASM_BYTES + 1)
        except zstd.ZstdError as error:
            raise ValueError("invalid SDK-compressed bootstrap") from error
        need(len(decoded) <= MAX_WASM_BYTES, "decompressed bootstrap Wasm size bound")
        need(decoder.eof and not decoder.unused_data,
             "SDK-compressed bootstrap must be one complete frame without trailing bytes")
    need(decoded.startswith(b"\0asm\x01\0\0\0"), "bootstrap must decode to a Wasm module")


def template_inputs(spec, config, bootstrap):
    need(isinstance(spec, dict), "chain spec must be an object")
    exact(config, CONFIG_FIELDS)
    authority_json(config)
    need(type(config["schemaVersion"]) is int and config["schemaVersion"] == 3
         and config["checkpointHash"] == CHECKPOINT_HASH, "recovery checkpoint identity")
    need(type(config["previousGrandpaSetId"]) is int and config["previousGrandpaSetId"] == 7956,
         "recovery checkpoint GRANDPA set differs")
    need(config["grandpaAuthorities"] == config["previousGrandpaAuthorities"],
         "checkpoint GRANDPA authority keys/order/weights must be preserved")
    approved = [authority_key(key) for key in config["approvedGrandpaKeys"]]
    active = [authority_key(row[0]) for row in config["grandpaAuthorities"]]
    need(approved == sorted(set(approved)) and len(active) == len(set(active))
         and set(active).issubset(approved) and all(any(key) for key in approved),
         "checkpoint GRANDPA retained-key inventory differs")
    need(type(config["lastObservedAbandonedSlot"]) is int and type(config["firstRecoverySlot"]) is int
         and config["lastObservedAbandonedSlot"] == 298871044 < config["firstRecoverySlot"] < 2**64,
         "public abandoned-chain slot bound differs")
    need(spec.get("id") == "sora-nexus-recovery-28191817"
         and spec.get("protocolId") == "sora-nexus-recovery-28191817-v1"
         and spec.get("forkId") == "recovery-28191817-v1", "recovery network identity")
    # These are an offline template, not an approved operator discovery configuration.
    need(spec.get("bootNodes") == [] and spec.get("telemetryEndpoints") == [],
         "offline template must have empty discovery and telemetry lists")
    need(spec.get("recovery") == config, "config differs from chain extension")
    from release_package import bounded_hex, policy_identity
    policy = policy_identity(bounded_hex(config["policyScale"], 2 * 1024**2))
    need(policy["authorizationDomain"]["chainSpecId"] == spec["id"]
         and config["policyCommitment"] == "0x" + policy["commitment"]
         and config["evidenceCommitment"] == "0x" + policy["evidenceCommitment"],
         "recovery policy/domain differs from chain specification")
    validate_bootstrap(bootstrap)
    code_hash = "0x" + hashlib.blake2b(bootstrap, digest_size=32).hexdigest()
    need(config["bootstrapCodeHash"] == code_hash, "bootstrap code hash mismatch")
    need(spec.get("codeSubstitutes", {}).get("28191817") == "0x" + bootstrap.hex(),
         "checkpoint substitute bytes mismatch")
    raw = spec.get("genesis", {}).get("raw")
    need(isinstance(raw, dict) and isinstance(raw.get("top"), dict)
         and isinstance(raw.get("childrenDefault"), dict), "raw genesis required")
    return code_hash


def verify(manifest_path, output):
    manifest_path, _ = physical(manifest_path)
    manifest_pin = file_pin(manifest_path, 1024**2)
    manifest = strict_json(read_bounded(manifest_path, 1024**2))
    exact(manifest, {"schema", "artifacts"})
    need(manifest["schema"] == SCHEMA, "unsupported node identity schema")
    exact(manifest["artifacts"], ROLES)
    sources, pins = {}, {}
    for role, maximum in ROLES.items():
        binding = manifest["artifacts"][role]
        exact(binding, {"file", "sha256", "bytes"})
        need(isinstance(binding["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", binding["sha256"])
             and type(binding["bytes"]) is int, "invalid artifact binding")
        sources[role] = relative(manifest_path.parent, binding["file"])
        pins[role] = file_pin(sources[role], maximum)
        need(pins[role] == {key: binding[key] for key in ("sha256", "bytes")},
             f"artifact binding mismatch: {role}")
    native_format(sources["nodeBinary"])
    need(sources["nodeBinary"].stat().st_mode & 0o111, "nodeBinary is not executable")
    need(len(set(sources.values())) == len(ROLES), "artifact roles must use distinct files")
    spec = strict_json(read_bounded(sources["chainSpec"], ROLES["chainSpec"]))
    config = strict_json(read_bounded(sources["recoveryConfig"], ROLES["recoveryConfig"]))
    code_hash = template_inputs(spec, config, read_bounded(sources["bootstrapWasm"], ROLES["bootstrapWasm"]))
    output = Path(output).absolute()
    physical(output.parent, directory=True)
    need(not output.exists() and not output.is_symlink(), "output must be a new directory")
    output.mkdir(mode=0o700)
    try:
        copied = {}
        for role, source in sources.items():
            destination = output / COPY_NAMES[role]
            copied_bytes = 0
            with source.open("rb") as src, destination.open("xb") as dst:
                while block := src.read(1024**2):
                    copied_bytes += len(block)
                    need(copied_bytes <= ROLES[role], "copy artifact bound")
                    dst.write(block)
            destination.chmod(0o500 if role == "nodeBinary" else 0o400)
            need(file_pin(destination, ROLES[role]) == pins[role], "copied artifact mismatch")
            copied[role] = destination
        for directory in ("tmp", "config", "cache", "data"):
            (output / directory).mkdir(mode=0o700)
        environment = {"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
                       "TMPDIR": str(output / "tmp"),
                       "XDG_CONFIG_HOME": str(output / "config"), "XDG_CACHE_HOME": str(output / "cache"),
                       "XDG_DATA_HOME": str(output / "data")}

        def check_pins():
            need(file_pin(manifest_path, 1024**2) == manifest_pin, "manifest changed")
            for role in ROLES:
                need(file_pin(sources[role], ROLES[role]) == pins[role]
                     and file_pin(copied[role], ROLES[role]) == pins[role], "artifact changed")

        check_pins()
        version_receipt, version_bytes = run_fixed([str(copied["nodeBinary"]), "--version"],
                                                   output, "version", environment, VERSION_TIMEOUT_SECONDS)
        need(re.fullmatch(rb"framenode 4\.8\.13-[A-Za-z0-9][A-Za-z0-9._+\-]{0,180}\n{1,2}", version_bytes),
             "unexpected executable version stdout")
        check_pins()
        generated = output / "prepared.json"
        prepare_receipt, prepare_stdout = run_fixed([
            str(copied["nodeBinary"]), "prepare-recovery-spec", "--recovery-config",
            str(copied["recoveryConfig"]), "--bootstrap-wasm", str(copied["bootstrapWasm"]),
            "--output", str(generated)], output, "prepare", environment, PREPARE_TIMEOUT_SECONDS)
        need(not prepare_stdout, "unexpected prepare command stdout")
        generated_pin = file_pin(generated, ROLES["chainSpec"])
        regenerated = strict_json(read_bounded(generated, ROLES["chainSpec"]))
        need(json.dumps(regenerated, sort_keys=True, separators=(",", ":"), allow_nan=False)
             == json.dumps(spec, sort_keys=True, separators=(",", ":"), allow_nan=False),
             "prepared template differs from candidate chain spec")
        template_inputs(regenerated, config, read_bounded(copied["bootstrapWasm"], ROLES["bootstrapWasm"]))
        check_pins()
        report = {"schema": SCHEMA + "-result", "manifest": manifest_pin, "artifacts": pins,
                  "checkpointHash": CHECKPOINT_HASH, "bootstrapCodeHash": code_hash,
                  "nodeVersion": "4.8.13", "versionStdout": version_bytes.decode("ascii"),
                  "versionStdoutSha256": sha(version_bytes), "preparedTemplate": generated_pin,
                  "processes": [version_receipt, prepare_receipt],
                  "executableVersionObserved": True, "exactOfflineTemplateReproduced": True,
                  "nodeSourceBuildAuthenticityVerified": False, "productionNetworkAdmissionVerified": False,
                  "databaseStartupVerified": False, "consensusReadinessVerified": False,
                  "operatorInventoriesApproved": False, "noNetworkSandboxEnforced": False,
                  "releaseGatePromoted": False, "productionReady": False}
        write_json(output / "result.json", report)
        return report
    except Exception as error:
        write_json(output / "failure.json", {"schema": SCHEMA + "-failure",
                   "error": str(error), "productionReady": False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = verify(args.manifest, args.output)
    print(json.dumps({key: report[key] for key in ("nodeVersion", "exactOfflineTemplateReproduced", "productionReady")}))


if __name__ == "__main__":
    main()
