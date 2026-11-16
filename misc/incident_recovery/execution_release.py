#!/usr/bin/env python3
"""Offline completed-history archive binding; never runtime execution or approval.

The first supported kind is the existing selected-step historical replay format.
The supported build/pin formats are public fixtures and stay diagnostic support.
Other kinds and production claims fail closed. All code here is owned, fixed code;
no evidence-supplied Python, command, plugin or module is imported or executed.
"""
from __future__ import annotations

import argparse
import datetime
import gzip
import hashlib
import io
import json
import math
from pathlib import Path, PurePosixPath
import re
import stat

SCHEMA = "sora-nexus-completed-history-execution-index-v1"
CHECKPOINT = 28191817
CHECKPOINT_HASH = "0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0"
CHECKPOINT_ROOT = "0x79bda41a1db53470463653f358890f2d1fca8ad622c33a1d7095294b0b140452"
MAX_JSON = 64 * 1024**2
MAX_SHARD = 512 * 1024**2
MAX_TOTAL = 8 * 1024**3
MAX_OBJECTS = 100_000
PREFIX = "executionArchiveShard"
ROLE = "executionEvidenceIndex"
COHORT_ROLES = {"bootstrapWasm", "policyScale", "operatorsScale", "signedMappingCsv", "checkpointCaptureManifest", "bootstrapProvenance"}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def blake(data):
    return "0x" + hashlib.blake2b(data, digest_size=32).hexdigest()


def digest(value):
    need(isinstance(value, str) and re.fullmatch("[0-9a-f]{64}", value), "invalid SHA256")
    return value


def strict_json(data, maximum=MAX_JSON):
    need(len(data) <= maximum, "JSON bound")
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, "duplicate JSON key")
            result[key] = value
        return result
    def finite(value):
        result = float(value)
        need(math.isfinite(result), "nonfinite JSON number")
        return result
    return json.loads(data, object_pairs_hook=pairs, parse_float=finite, parse_constant=lambda _: need(False, "nonfinite JSON"))


def keys(value, expected):
    need(isinstance(value, dict) and set(value) == set(expected), "unexpected fields")


def canonical(value, absolute=False):
    need(isinstance(value, str) and value and not any(c in value for c in "\\\n\r\x00"), "invalid path")
    path = PurePosixPath(value)
    need(path.is_absolute() is absolute and str(path) == value and all(p not in (".", "..") for p in path.parts), "noncanonical path")
    return path


def joined(base, relative):
    canonical(relative)
    return str(PurePosixPath(base) / relative)


def physical(path):
    path = Path(path).absolute()
    need(str(path) == str(canonical(str(path), absolute=True)), "physical path spelling")
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        info = current.lstat()
        need(not stat.S_ISLNK(info.st_mode), "symlink evidence")
    need(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, "regular unaliased file required")
    return info


class Archive:
    """Content-addressed shards, with absolute original paths used only as labels."""
    def __init__(self, root, archive, role_paths=None):
        keys(archive, {"shards", "objects"})
        self.root = Path(root).absolute()
        self.used = set()
        self.objects = archive["objects"]
        need(isinstance(self.objects, dict) and 0 < len(self.objects) <= MAX_OBJECTS, "object inventory bound")
        need(isinstance(archive["shards"], list) and 0 < len(archive["shards"]) <= 32, "shard bound")
        self.members, self.files, self.pins = {}, {}, []
        total = 0
        for index, shard in enumerate(archive["shards"]):
            keys(shard, {"role", "file", "sha256", "bytes", "members"})
            need(shard["role"] == f"{PREFIX}{index:04d}", "noncanonical shard role")
            path = self.root / str(canonical(shard["file"]))
            if role_paths is not None:
                need(shard["role"] in role_paths, "shard not bound as candidate artifact")
                # Packaged roles resolve to content-addressed objects. The old
                # relative filename is a canonical label, never a fallback.
                path = Path(role_paths[shard["role"]]).absolute()
            info = physical(path)
            need(type(shard["bytes"]) is int and 0 <= shard["bytes"] <= MAX_SHARD and info.st_size == shard["bytes"], "shard size")
            total += info.st_size
            need(total <= MAX_TOTAL, "aggregate archive bound")
            with path.open("rb") as stream:
                need(hashlib.file_digest(stream, "sha256").hexdigest() == digest(shard["sha256"]), "shard hash mismatch")
            need(isinstance(shard["members"], list) and 0 < len(shard["members"]) <= MAX_OBJECTS, "member inventory bound")
            offset = 0
            with path.open("rb") as stream:
                for member in shard["members"]:
                    keys(member, {"sha256", "bytes", "offset"})
                    identity = digest(member["sha256"])
                    need(identity not in self.members and type(member["bytes"]) is int and member["bytes"] >= 0 and type(member["offset"]) is int and member["offset"] == offset, "duplicate/gapped archive member")
                    remaining = member["bytes"]
                    need(offset + remaining <= info.st_size, "member outside shard")
                    h = hashlib.sha256()
                    while remaining:
                        data = stream.read(min(1024**2, remaining))
                        need(data, "truncated member")
                        remaining -= len(data)
                        h.update(data)
                    need(h.hexdigest() == identity, "member hash mismatch")
                    self.members[identity] = (path, offset, member["bytes"])
                    offset += member["bytes"]
            need(offset == info.st_size, "unbound shard suffix")
            self.files[shard["role"]] = path
            self.pins.append({k: shard[k] for k in ("role", "file", "sha256", "bytes")})
        referenced = set()
        for name, record in self.objects.items():
            canonical(name, absolute=True)
            keys(record, {"sha256", "bytes"})
            identity = digest(record["sha256"])
            need(type(record["bytes"]) is int and identity in self.members and self.members[identity][2] == record["bytes"], "unbound logical object")
            referenced.add(identity)
        need(referenced == set(self.members), "extra unbound archive member")

    def read(self, label, expected=None, size=None, maximum=MAX_JSON):
        canonical(label, absolute=True)
        need(label in self.objects, "missing archived object: " + label)
        record = self.objects[label]
        need(expected is None or record["sha256"] == digest(expected), "object hash binding")
        need(size is None or record["bytes"] == size, "object length binding")
        path, offset, length = self.members[record["sha256"]]
        need(length <= maximum, "object read bound")
        physical(path)
        with path.open("rb") as stream:
            stream.seek(offset)
            data = stream.read(length)
        need(len(data) == length and sha(data) == record["sha256"], "archive object changed")
        self.used.add(label)
        return data

    def pin(self, label, expected=None, size=None):
        # Large Wasm/executable/source objects are rehashed without buffering.
        canonical(label, absolute=True)
        need(label in self.objects, "missing archived object: " + label)
        record = self.objects[label]
        need(expected is None or record["sha256"] == digest(expected), "object hash binding")
        need(size is None or record["bytes"] == size, "object length binding")
        path, offset, remaining = self.members[record["sha256"]]
        physical(path)
        h = hashlib.sha256()
        with path.open("rb") as stream:
            stream.seek(offset)
            while remaining:
                data = stream.read(min(1024**2, remaining))
                need(data, "truncated archived object")
                remaining -= len(data)
                h.update(data)
        need(h.hexdigest() == record["sha256"], "archived object changed")
        self.used.add(label)
        return record

    def json(self, label, expected=None, maximum=MAX_JSON):
        return strict_json(self.read(label, expected, maximum=maximum), maximum)

    def close(self):
        need(self.used == set(self.objects), "extra unvisited logical archive objects")
        for pin in self.pins:
            path = self.files[pin["role"]]
            need(physical(path).st_size == pin["bytes"], "shard changed")
            with path.open("rb") as stream:
                need(hashlib.file_digest(stream, "sha256").hexdigest() == pin["sha256"], "shard changed")


# The bounded SCALE/header logic is adapted from the fixed owned v15 report
# validator; source hashes and scope are retained in this candidate's REUSE.json.
def compact(value):
    need(type(value) is int and 0 <= value < 2**30, "bounded compact")
    if value < 64:
        return bytes([value * 4])
    if value < 16384:
        return (value * 4 + 1).to_bytes(2, "little")
    return (value * 4 + 2).to_bytes(4, "little")


class Scale:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def take(self, size):
        need(type(size) is int and 0 <= size <= len(self.data) - self.pos, "truncated SCALE")
        value = self.data[self.pos:self.pos + size]
        self.pos += size
        return value

    def integer(self):
        first = self.take(1)[0]
        mode = first & 3
        need(mode != 3, "unsupported large compact")
        raw = bytes([first]) + self.take((1 << mode) - 1)
        value = int.from_bytes(raw, "little") >> 2
        need(raw == compact(value), "noncanonical compact")
        return value

    def vector(self):
        return self.take(self.integer())


def unhex(value, size=None):
    need(isinstance(value, str) and re.fullmatch("0x(?:[0-9a-f]{2})*", value), "canonical hex required")
    data = bytes.fromhex(value[2:])
    need(size is None or len(data) == size, "wrong fixed hex length")
    return data


def rpc_header(value):
    keys(value, {"parentHash", "number", "stateRoot", "extrinsicsRoot", "digest"})
    keys(value["digest"], {"logs"})
    need(isinstance(value["number"], str) and re.fullmatch("0x[0-9a-f]+", value["number"]), "header number")
    number = int(value["number"], 16)
    need(0 <= number <= CHECKPOINT and value["number"] == hex(number), "header number spelling/bound")
    logs = value["digest"]["logs"]
    need(isinstance(logs, list) and len(logs) <= 1024, "digest bound")
    data = unhex(value["parentHash"], 32) + compact(number) + unhex(value["stateRoot"], 32) + unhex(value["extrinsicsRoot"], 32) + compact(len(logs))
    for item in logs:
        raw = unhex(item)
        need(len(raw) <= 1024**2, "digest byte bound")
        reader = Scale(raw)
        tag = reader.take(1)[0]
        if tag in (4, 5, 6):
            reader.take(4)
            reader.vector()
        elif tag == 0:
            reader.vector()
        else:
            need(tag == 8, "unknown digest item")
        need(reader.pos == len(raw), "trailing digest bytes")
        data += raw
    return {"number": number, "hash": blake(data), "parent": value["parentHash"], "root": value["stateRoot"], "scale": data}


def pin_reference(archive, base, reference, maximum=MAX_JSON):
    keys(reference, {"file", "sha256"})
    label = joined(base, reference["file"])
    return label, archive.read(label, reference["sha256"], maximum=maximum)


def completed_step(archive, run):
    started, result = archive.json(run["started"]), archive.json(run["completed"])
    plan = archive.json(run["plan"])
    need(plan.get("schema") == "sora-nexus-v15-bound-orchestration-v1" and plan.get("fixtureOnly") is True and plan.get("productionReady") is False, "unsupported recorder plan/scope")
    steps = plan.get("steps")
    need(isinstance(steps, list) and 0 < len(steps) <= 512, "plan step bound")
    ids = [step.get("id") for step in steps]
    need(len(ids) == len(set(ids)) and run["step"] in ids, "missing/duplicate plan step")
    step = steps[ids.index(run["step"])]
    expected = [run["verifier"], "replay-history", "--manifest", run["manifest"]]
    need(step["command"] == expected and step["lane"] == "wasm-execution" and step["outputs"] == [], "wrong historical command/selector")
    common = {"schema", "step", "command", "cwd", "startedAt", "controllerPid", "planSha256", "productionReady", "inputFilesBefore", "dependencyReceipts", "sourceMutationLane", "outputsAbsentBefore", "pid", "processGroup"}
    need(common <= started.keys() and common <= result.keys(), "missing process fields")
    for key in common:
        need(started[key] == result[key], "started/completed process mismatch")
    need(result["schema"] == "sora-nexus-root-selected-step-v1" and result["step"] == run["step"] and result["command"] == expected and result["cwd"] == step["cwd"], "process identity/command mismatch")
    need(result["planSha256"] == archive.pin(run["plan"])["sha256"], "process plan mismatch")
    need(type(result["pid"]) is int and result["pid"] > 0 and result["processGroup"] == result["pid"] and type(result["controllerPid"]) is int and result["controllerPid"] > 0, "process identity absent")
    need(result["productionReady"] is False and result["sourceMutationLane"] is False and result["outputsAbsentBefore"] == [], "process scope mismatch")
    need(type(result.get("exitCode")) is int and result["exitCode"] == 0 and result.get("status") == "passed" and result.get("inputChanges") == [] and result.get("missingOutputs") == [] and result.get("planUnchanged") is True, "failed/incomplete execution")
    start = datetime.datetime.fromisoformat(result["startedAt"])
    end = datetime.datetime.fromisoformat(result["finishedAt"])
    need(start.tzinfo is not None and end.tzinfo is not None and end >= start and type(result["elapsedSeconds"]) in (int, float) and result["elapsedSeconds"] >= 0, "invalid execution interval")
    before = result["inputFilesBefore"]
    need(isinstance(before, dict) and before and before == result.get("inputFilesAfter"), "input set changed or absent")
    need({run["verifier"], run["manifest"]} <= before.keys(), "executed binary/manifest missing")
    for label, item in before.items():
        need(item["path"] == label and item["resolvedPath"] == label, "aliased recorded input")
        archive.pin(label, item["sha256"], item["bytes"])
    dependencies = result["dependencyReceipts"]
    need(isinstance(dependencies, list) and len(dependencies) == len(step["dependsOn"]), "dependency receipt coverage")
    expected_dependencies = [str(PurePosixPath(run["completed"]).parent.parent / name / "execution.json") for name in step["dependsOn"]]
    need([ref["path"] for ref in dependencies] == expected_dependencies, "wrong predecessor receipt")
    for ref in dependencies:
        need(ref["resolvedPath"] == ref["path"], "dependency alias")
        archive.pin(ref["path"], ref["sha256"], ref["bytes"])
        dependency = archive.json(ref["path"], ref["sha256"])
        need(type(dependency.get("exitCode")) is int and dependency["exitCode"] == 0 and dependency.get("status") == "passed" and dependency.get("planSha256") == result["planSha256"], "incomplete/foreign dependency receipt")
    directory = PurePosixPath(run["completed"]).parent
    need(directory.name == run["step"] and PurePosixPath(run["completed"]).name == "execution.json"
         and PurePosixPath(run["started"]) == directory / "started.json"
         and PurePosixPath(run["stderr"]) == directory / "stderr.log", "process receipt paths differ")
    stdout = archive.read(run["stdout"], result["stdoutSha256"])
    archive.read(run["stderr"], result["stderrSha256"])
    need(str(PurePosixPath(run["stdout"]).parent) == str(PurePosixPath(run["completed"]).parent) and PurePosixPath(run["stdout"]).name == "stdout.log" and PurePosixPath(run["stderr"]).name == "stderr.log", "report/process output path differs")
    return strict_json(stdout), result


MAX_ANCESTRY = 128 * 1024**2
MAX_HEADERS = 262_144
MAX_CAPTURES = 32
CAPTURE_ARTIFACTS = {"checkpoint-header.json", "parent-header.json", "checkpoint-block.json", "runtime-version.json", "runtime-metadata.scale", "runtime-code.wasm"}


def capture_closure(archive, label, expected=None):
    capture = archive.json(label, expected)
    need(capture.get("schema") == "sora-nexus-checkpoint-capture-v1" and capture.get("complete") is True,
         "unsupported or incomplete capture")
    base = str(PurePosixPath(label).parent)
    artifacts = capture.get("artifacts")
    need(isinstance(artifacts, dict) and set(artifacts) == CAPTURE_ARTIFACTS, "capture artifact set differs")
    for name, record in artifacts.items():
        keys(record, {"sha256", "bytes"})
        archive.pin(joined(base, name), record["sha256"], record["bytes"])
    header = rpc_header(archive.json(joined(base, "checkpoint-header.json")))
    parent = rpc_header(archive.json(joined(base, "parent-header.json")))
    checkpoint = capture["checkpoint"]
    need(checkpoint["number"] == header["number"] and checkpoint["hash"] == header["hash"]
         and checkpoint["parentHash"] == header["parent"] and checkpoint["stateRoot"] == header["root"]
         and checkpoint["runtimeCodeSha256"] == artifacts["runtime-code.wasm"]["sha256"]
         and parent["hash"] == header["parent"] and parent["number"] + 1 == header["number"], "capture header identity differs")
    block = archive.json(joined(base, "checkpoint-block.json"))
    need(rpc_header(block["block"]["header"]) == header, "capture block/header mismatch")
    children = capture.get("children")
    need(isinstance(children, list) and len(children) <= 4096, "child trie bound")
    pages_seen = set()
    for trie in [capture["top"], *children]:
        need(isinstance(trie, dict) and isinstance(trie.get("pages"), list)
             and type(trie.get("pageCount")) is int and trie["pageCount"] == len(trie["pages"])
             and 0 <= len(trie["pages"]) <= 16384, "capture page count")
        total = 0
        previous = None
        for page_index, page in enumerate(trie["pages"]):
            keys(page, {"file", "sha256", "keyCount", "firstKey", "lastKey"})
            name = joined(base, page["file"])
            need(name not in pages_seen, "duplicate capture page")
            pages_seen.add(name)
            need(type(page["keyCount"]) is int and 0 <= page["keyCount"] <= 4096, "page key count bound")
            if page["keyCount"] == 0:
                need(page_index == len(trie["pages"]) - 1 and page["firstKey"] is None
                     and page["lastKey"] == (None if previous is None else "0x" + previous.hex()),
                     "invalid terminal empty capture page")
                compressed = archive.read(name, page["sha256"], maximum=1024**2)
                with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as stream:
                    plain = stream.read(65537)
                need(len(plain) <= 65536, "terminal page decompression bound")
                lines = [line for line in plain.splitlines() if line]
                need(len(lines) == 1, "terminal capture page has data rows")
                terminal = strict_json(lines[0], 65536)
                need(terminal.get("schema") == "sora-nexus-checkpoint-page-v1"
                     and terminal.get("checkpointHash") == checkpoint["hash"]
                     and terminal.get("terminal") is True and type(terminal.get("keyCount")) is int
                     and terminal["keyCount"] == 0 and terminal.get("pageIndex") == page_index
                     and terminal.get("startKey") == page["lastKey"] and terminal.get("lastKey") == page["lastKey"],
                     "terminal capture page header differs")
                continue
            first, last = unhex(page["firstKey"]), unhex(page["lastKey"])
            need(first <= last and (previous is None or previous < first), "overlapping capture page keys")
            previous = last
            total += page["keyCount"]
            archive.pin(name, page["sha256"])
        need(type(trie.get("keyCount")) is int and total == trie["keyCount"], "capture aggregate key count")
    # These are artifact-closure checks. The pinned Rust replay verifies trie
    # roots; this Python verifier does not parse/recompute the state trie.
    return header, len(pages_seen)


def validate_history(archive, run):
    keys(run, {"started", "completed", "stdout", "stderr", "plan", "step", "verifier", "manifest"})
    report, receipt = completed_step(archive, run)
    manifest = archive.json(run["manifest"])
    need(manifest.get("schema") == "sora-nexus-historical-replay-v1"
         and manifest.get("checkpointHash") == CHECKPOINT_HASH, "historical manifest identity")
    keys(manifest, {"schema", "checkpointHash", "candidate", "ancestry", "ranges"})
    need(report.get("schema") == "sora-nexus-historical-replay-witness-v1"
         and report.get("checkpointHash") == CHECKPOINT_HASH
         and report.get("manifestSha256") == archive.pin(run["manifest"])["sha256"]
         and report.get("consensusSealsVerified") is False and report.get("productionReady") is False,
         "historical report identity/scope")
    base = str(PurePosixPath(run["manifest"]).parent)
    candidate_label, candidate = pin_reference(archive, base, manifest["candidate"])
    need(report.get("candidateSha256") == sha(candidate) and report.get("candidateBlake2") == blake(candidate),
         "executed candidate differs")
    ancestry_label = joined(base, manifest["ancestry"]["file"])
    keys(manifest["ancestry"], {"file", "sha256"})
    ancestry = archive.json(ancestry_label, manifest["ancestry"]["sha256"], MAX_ANCESTRY)
    need(isinstance(ancestry, list) and 0 < len(ancestry) <= MAX_HEADERS, "ancestry bound")
    anchored = {}
    previous = None
    for item in ancestry:
        header = rpc_header(item)
        need(header["number"] not in anchored, "duplicate ancestry header")
        if previous is not None:
            need(header["number"] == previous["number"] + 1 and header["parent"] == previous["hash"], "broken ancestry")
        anchored[header["number"]] = header
        previous = header
    need(previous["number"] == CHECKPOINT and previous["hash"] == CHECKPOINT_HASH
         and previous["root"] == CHECKPOINT_ROOT and report.get("ancestrySha256") == manifest["ancestry"]["sha256"],
         "ancestry not anchored to fixed checkpoint")
    ranges = manifest["ranges"]
    need(isinstance(ranges, list) and 0 < len(ranges) <= MAX_CAPTURES and isinstance(report.get("ranges"), list)
         and len(ranges) == len(report["ranges"]), "history range count")
    names, seen_blocks, coverage, count, pages = set(), set(), set(), 0, 0
    deployed_codes, deployed_claims = {}, []
    for planned, observed in zip(ranges, report["ranges"]):
        keys(planned, {"name", "rationale", "boundaries", "parentCapture", "blocks"})
        need(isinstance(planned["name"], str) and planned["name"] and planned["name"] not in names,
             "duplicate/empty history range")
        names.add(planned["name"])
        for field in ("name", "rationale", "boundaries"):
            need(observed.get(field) == planned[field], "range declaration/report mismatch")
        boundaries = planned["boundaries"]
        need(isinstance(boundaries, list) and boundaries and len(set(boundaries)) == len(boundaries)
             and set(boundaries) <= {"checkpoint", "session", "epoch", "upgrade"}, "range boundaries")
        need(isinstance(planned["blocks"], list) and 0 < len(planned["blocks"]) <= 4096
             and len(observed.get("blocks", [])) == len(planned["blocks"]), "range block count")
        parent_label = joined(base, planned["parentCapture"] + "/capture-complete.json")
        parent, num_pages = capture_closure(archive, parent_label, observed["parentCaptureManifestSha256"])
        need(anchored.get(parent["number"]) == parent, "capture parent absent from anchored ancestry")
        capture_base = str(PurePosixPath(parent_label).parent)
        captured_spec = archive.json(joined(capture_base, "runtime-version.json")).get("specVersion")
        need(type(captured_spec) is int and captured_spec in (133, 134), "capture runtime spec differs")
        code_hash = blake(archive.read(joined(capture_base, "runtime-code.wasm"), maximum=50 * 1024**2))
        need(captured_spec not in deployed_codes or deployed_codes[captured_spec] == code_hash, "conflicting deployed code for historical spec")
        deployed_codes[captured_spec] = code_hash
        need(observed["blocks"][0].get("deployedSpec") == captured_spec, "first execution spec differs from captured parent")
        pages += num_pages
        observed_boundaries = set()
        for ref, block in zip(planned["blocks"], observed["blocks"]):
            _, raw = pin_reference(archive, base, ref)
            rpc = strict_json(raw)
            need(isinstance(rpc.get("block"), dict) and isinstance(rpc["block"].get("extrinsics"), list), "RPC block shape")
            for extrinsic in rpc["block"]["extrinsics"]:
                encoded = unhex(extrinsic)
                need(len(encoded) <= 16 * 1024**2, "extrinsic bound")
                reader = Scale(encoded)
                need(reader.integer() == len(encoded) - reader.pos, "extrinsic length prefix")
            header = rpc_header(rpc["block"]["header"])
            need(header == anchored.get(header["number"]) and header["number"] not in seen_blocks,
                 "block not in anchored history or repeated")
            need(header["number"] == parent["number"] + 1 and header["parent"] == parent["hash"], "nonconsecutive replay range")
            need(block.get("number") == header["number"] and block.get("hash") == header["hash"]
                 and block.get("parentHash") == parent["hash"] and block.get("parentStateRoot") == parent["root"]
                 and block.get("postStateRoot") == header["root"], "report/header root mismatch")
            need(block.get("identicalStateRoots") is True and block.get("nativeExecution") is False,
                 "replay receipt does not assert Wasm root equality")
            spec = block.get("deployedSpec")
            need(type(spec) is int and spec in (133, 134), "unreviewed historical spec")
            need(block.get("substituteApplied") is (spec == 134), "substitution cutoff differs")
            unhex(block.get("deployedExecutorHash"), 32)
            deployed_claims.append((spec, block["deployedExecutorHash"]))
            need(block.get("candidateExecutorHash") == (blake(candidate) if spec == 134 else block["deployedExecutorHash"]),
                 "historical executor hash mismatch")
            found = block.get("boundaries")
            need(isinstance(found, list) and len(set(found)) == len(found)
                 and set(found) <= set(boundaries), "unexpected observed boundary")
            if "checkpoint" in found:
                need(header["number"] == CHECKPOINT, "false checkpoint boundary")
            observed_boundaries.update(found)
            seen_blocks.add(header["number"])
            parent = header
            count += 1
        need(observed_boundaries == set(boundaries), "missing declared boundary observation")
        coverage.update(observed_boundaries)
    need(all(deployed_codes.get(spec) == code_hash for spec, code_hash in deployed_claims), "deployed executor differs from captured code")
    need(coverage == {"checkpoint", "session", "epoch", "upgrade"}
         and report.get("representativeBoundaryCoverage") is True, "incomplete representative boundary coverage")
    return {"blocks": count, "ranges": len(ranges), "ancestryHeaders": len(anchored), "capturePages": pages,
            "candidateLabel": candidate_label, "candidateSha256": sha(candidate),
            "nativeReceiptSha256": archive.pin(run["completed"])["sha256"],
            "nativeReportSha256": archive.pin(run["stdout"])["sha256"]}


def validate_cohort(archive, index, history):
    cohort = index["cohort"]
    keys(cohort, COHORT_ROLES)
    for role, record in cohort.items():
        keys(record, {"label", "sha256", "bytes"})
        archive.pin(record["label"], record["sha256"], record["bytes"])
    need(cohort["bootstrapWasm"]["sha256"] == history["candidateSha256"], "cohort bootstrap differs from executed bytes")
    build = archive.json(index["bootstrapBuild"])
    need(build.get("schema") == "sora-nexus-fixture-wasm-build-v1" and build.get("fixtureOnly") is True
         and build.get("productionReady") is False and build.get("status") == "passed"
         and type(build.get("exitCode")) is int and build["exitCode"] == 0
         and build.get("specVersion") == 134 and build.get("transactionVersion") == 131,
         "unreviewed/failed bootstrap build or production scope")
    name = "runtime-134.compact.compressed.wasm"
    need(build["artifacts"][name]["sha256"] == history["candidateSha256"], "bootstrap build bytes differ")
    sources = archive.json(index["bootstrapSources"], build["sourceFingerprint"]["sha256"])
    need(isinstance(sources, dict) and 0 < len(sources) <= 16384, "bootstrap source inventory bound")
    for name, record in sources.items():
        archive.pin(joined(build["source"], name), record["sha256"], record["bytes"])
    provenance = archive.json(cohort["bootstrapProvenance"]["label"], build["sourceProvenance"]["sha256"])
    need(provenance.get("fixtureOnly") is True and provenance.get("releaseReady") is False
         and provenance.get("bound") is True and provenance.get("specVersion") == 134
         and provenance.get("transactionVersion") == 131, "unreviewed bootstrap provenance")
    for field, role in (("policy", "policyScale"), ("operators", "operatorsScale")):
        need(provenance["bindingEvidence"][field]["sha256"] == cohort[role]["sha256"], "bound bootstrap policy/operators differ")
    build_base = str(PurePosixPath(index["bootstrapBuild"]).parent)
    for name, record in build["logs"].items():
        archive.pin(joined(build_base, name), record["sha256"], record["bytes"])
    verifier = archive.json(index["verifierPin"])
    need(verifier.get("schema") == "sora-nexus-public-verifier-pin-v1" and verifier.get("fixtureOnly") is True
         and verifier.get("productionReady") is False, "unreviewed verifier provenance")
    archive.pin(index["run"]["verifier"], verifier["sha256"], verifier["bytes"])
    native = archive.json(index["verifierBuildReceipt"])
    started = archive.json(index["verifierBuildStarted"])
    need(type(native.get("exitCode")) is int and native["exitCode"] == 0 and native.get("inputChanges") == []
         and native.get("sourceBefore") == native.get("sourceAfter") == verifier["sourceFiles"]
         and native.get("productionReady") is False, "verifier native build failed/changed")
    for field in ("command", "pid", "startedAt", "sourceBefore", "controlledEnvironment"):
        need(native.get(field) == started.get(field), "native build process join differs")
    command = native["command"]
    need(command == [joined(index["verifierSourceRoot"], "scripts/with_llvm_env.sh"), "cargo", "build", "--offline", "--locked", "-p", "incident-recovery", "--features", "bridge-fixture"], "unreviewed verifier build command")
    need(native["cwd"] == index["verifierSourceRoot"] and type(native["pid"]) is int and native["pid"] > 0,
         "verifier build cwd/process identity")
    need(native["log"] == index["verifierBuildLog"], "native build log path")
    archive.pin(index["verifierBuildLog"], native["logSha256"])
    need(Path(index["verifierBuildReceipt"]).name == verifier["actualCommandReceipts"]["build"], "verifier build receipt selection")
    for name, expected in verifier["sourceFiles"].items():
        archive.pin(joined(index["verifierSourceRoot"], name), expected)
    capture, _ = capture_closure(archive, cohort["checkpointCaptureManifest"]["label"], cohort["checkpointCaptureManifest"]["sha256"])
    need(capture["number"] == CHECKPOINT and capture["hash"] == CHECKPOINT_HASH and capture["root"] == CHECKPOINT_ROOT,
         "cohort capture is not checkpoint")
    # CSV bytes are bound as a declared cohort input. Ownership signatures and
    # agreement with the compiled mapping remain the native ownership gate.
    return {role: record["sha256"] for role, record in cohort.items()}


def check_index(index, archive):
    keys(index, {"schema", "kind", "scope", "checkpointHash", "checkpointStateRoot", "run", "cohort",
                 "bootstrapBuild", "bootstrapSources", "verifierPin", "verifierBuildReceipt", "verifierBuildStarted",
                 "verifierBuildLog", "verifierSourceRoot", "archive"})
    need(index["schema"] == SCHEMA and index["kind"] == "history" and index["scope"] == "diagnostic-support",
         "unsupported execution kind/scope; production adapter is not implemented")
    need(index["checkpointHash"] == CHECKPOINT_HASH and index["checkpointStateRoot"] == CHECKPOINT_ROOT, "checkpoint differs")
    history = validate_history(archive, index["run"])
    cohort = validate_cohort(archive, index, history)
    archive.close()
    return {"schema": "sora-nexus-completed-history-transcript-verification-v1", "kind": "history",
            "scope": "diagnostic-support", "checkpointHash": CHECKPOINT_HASH, "checkpointStateRoot": CHECKPOINT_ROOT,
            "history": history, "cohortSha256": cohort, "archivedObjects": len(archive.objects),
            "completedTranscriptConsistent": True, "serializedHeadersAnchored": True,
            "captureObjectClosureVerified": True, "declaredSourceInventoryBound": True,
            "runtimeExecutionPerformed": False, "stateTrieRecomputed": False, "extrinsicsTrieRecomputed": False,
            "nativeReceiptAuthenticityIndependentlyProven": False,
            "observedBoundaryClaimsIndependentlyRecomputed": False,
            "consensusSealsVerified": False, "fullHistoryReexecuted": False,
            "accountOwnershipVerified": False, "externalFinalityVerified": False,
            "releaseGatePromoted": False, "productionReady": False}


def verify_file(path, expected_sha256, *, role_paths=None):
    source_pin = sha(Path(__file__).read_bytes())
    path = Path(path).absolute()
    need(physical(path).st_size <= MAX_JSON, "execution index bound")
    raw = path.read_bytes()
    need(sha(raw) == digest(expected_sha256), "execution index hash differs")
    index = strict_json(raw)
    archive = Archive(path.parent, index["archive"], role_paths)
    report = check_index(index, archive)
    need(path.read_bytes() == raw, "execution index changed during verification")
    report["indexSha256"] = sha(raw)
    need(sha(Path(__file__).read_bytes()) == source_pin, "verifier source changed during verification")
    report["verifierSourceSha256"] = source_pin
    return report


def package_hook(report, paths, expected_source_sha256, output):
    """Fixed owned module only. Adds scoped support; never clears a release gate."""
    need(sha(Path(__file__).read_bytes()) == digest(expected_source_sha256), "fixed execution verifier source differs")
    need(ROLE in paths, "missing execution evidence index")
    output = Path(output).absolute()
    need(not output.exists(), "execution verification output already exists")
    bindings = {row["role"]: row["sha256"] for row in report["artifacts"]}
    need(ROLE in bindings, "execution index not bound in candidate")
    output.mkdir(parents=True)
    started = {"schema": "sora-nexus-fixed-history-hook-execution-v1", "startedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "indexSha256": bindings[ROLE], "verifierSourceSha256": expected_source_sha256, "productionReady": False}
    (output / "started.json").write_text(json.dumps(started, indent=2) + "\n")
    try:
        result = verify_file(paths[ROLE], bindings[ROLE], role_paths=paths)
        # Every shard was read from a separately hash-bound candidate artifact.
        index = strict_json(Path(paths[ROLE]).read_bytes())
        for shard in index["archive"]["shards"]:
            need(bindings.get(shard["role"]) == shard["sha256"], "candidate shard digest differs")
        result["outerCandidateCohortMatches"] = {role: bindings.get(role) == value for role, value in result["cohortSha256"].items()}
        need(result["verifierSourceSha256"] == expected_source_sha256, "fixed verifier changed during hook")
        raw = (json.dumps(result, indent=2, sort_keys=True) + "\n").encode()
        (output / "report.json").write_bytes(raw)
        receipt = dict(started, finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(), exitCode=0,
                       reportSha256=sha(raw), status="passed")
        (output / "execution.json").write_text(json.dumps(receipt, indent=2) + "\n")
        (output / "VERIFICATION-COMPLETE").write_text(sha(raw) + "\n")
        report["executionSupportingFindings"] = result
        report["productionReady"] = False
        return result
    except BaseException as error:
        (output / "failure.json").write_text(json.dumps(dict(started, error=str(error), status="failed", sealed=False), indent=2) + "\n")
        raise


def verify_package(path):
    """Read roles from a copied package; never open the index's original paths."""
    path = Path(path).absolute()
    need(physical(path).st_size <= MAX_JSON, "package manifest bound")
    original = path.read_bytes()
    manifest = strict_json(original)
    need(manifest.get("format") == "sora-nexus-recovery-release-v1", "unknown release package")
    artifacts = manifest.get("artifacts")
    need(isinstance(artifacts, list) and 0 < len(artifacts) <= 16384, "package artifact bound")
    paths, bindings = {}, {}
    for row in artifacts:
        role = row["role"]
        need(isinstance(role, str) and role not in paths, "duplicate package role")
        obj = str(canonical(row["object"]))
        need(obj == "objects/" + digest(row["sha256"]), "package object identity")
        target = path.parent / obj
        need(type(row["bytes"]) is int and physical(target).st_size == row["bytes"], "package object length")
        paths[role], bindings[role] = target, row["sha256"]
    need(ROLE in paths, "package lacks execution index")
    result = verify_file(paths[ROLE], bindings[ROLE], role_paths=paths)
    index = strict_json(paths[ROLE].read_bytes())
    for shard in index["archive"]["shards"]:
        need(bindings.get(shard["role"]) == shard["sha256"], "packaged shard binding differs")
    need(path.read_bytes() == original, "package manifest changed")
    result["packageManifestSha256"] = sha(original)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--index", type=Path)
    inputs.add_argument("--release-manifest", type=Path)
    parser.add_argument("--index-sha256")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    need(not args.output.exists(), "output must be new")
    if args.index is not None:
        need(args.index_sha256 is not None, "pin the standalone execution index")
        result = verify_file(args.index, args.index_sha256)
    else:
        need(args.index_sha256 is None, "standalone index pin with packaged input")
        result = verify_package(args.release_manifest)
    with args.output.open("x") as stream:
        json.dump(result, stream, indent=2, sort_keys=True)
        stream.write("\n")
    print(json.dumps({"output": str(args.output), "scope": result["scope"], "productionReady": False}))


if __name__ == "__main__":
    main()
