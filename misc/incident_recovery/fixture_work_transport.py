#!/usr/bin/env python3
"""Bounded, hash-checked transport of public native-composition fixtures.

The native report is retained verbatim. Its envelope arrays are parsed one item
at a time and transported as individual SCALE files. Loading checks the complete
report against the index and every shard; the index is not execution evidence.
The general review-JSON reader's 64 MiB limit is deliberately unchanged.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath

import hashi_codec as scale
import release_package as release

SCHEMA = "sora-nexus-PUBLIC-FIXTURE-ONLY-storage-transport-v1"
MAX_REPORT_BYTES = 512 * 1024 * 1024
MAX_FIELD_BYTES = 64 * 1024 * 1024
MAX_ENVELOPE_BYTES = 576 * 1024
MAX_ENVELOPES = 16_384
READ_BYTES = 64 * 1024
ARRAYS = ("applyEnvelopes", "auditEnvelopes")
SPECIAL = {*ARRAYS, "terminalEnvelope", "policyScale"}


def artifact_limit(key):
    return release.MAX_POLICY if key == "policyScale" else MAX_ENVELOPE_BYTES


def unique(pairs):
    result = {}
    for key, value in pairs:
        scale.require(key not in result, "Duplicate JSON key")
        result[key] = value
    return result


def invalid_constant(value):
    raise ValueError(f"Non-JSON constant: {value}")


class ReportReader:
    """Incremental top-level decoder with bounded individual JSON values."""

    def __init__(self, stream):
        self.stream = stream
        self.buffer = ""
        self.eof = False
        self.read_bytes = 0
        self.decoder = json.JSONDecoder(object_pairs_hook=unique, parse_constant=invalid_constant)

    def fill(self):
        value = self.stream.read(READ_BYTES)
        self.read_bytes += len(value.encode("utf-8"))
        scale.require(self.read_bytes <= MAX_REPORT_BYTES, "Native fixture report exceeds bound")
        self.buffer += value
        self.eof = not value

    def whitespace(self):
        while True:
            self.buffer = self.buffer.lstrip(" \r\n\t")
            if self.buffer or self.eof:
                return
            self.fill()

    def take(self, token):
        self.whitespace()
        scale.require(self.buffer.startswith(token), f"Expected JSON delimiter {token}")
        self.buffer = self.buffer[len(token):]

    def value(self, limit=MAX_FIELD_BYTES):
        self.whitespace()
        while True:
            try:
                value, end = self.decoder.raw_decode(self.buffer)
                # A number can be a proper prefix of the next input chunk.
                if end == len(self.buffer) and not self.eof:
                    self.fill()
                    continue
                scale.require(len(self.buffer[:end].encode("utf-8")) <= limit,
                              "Native fixture JSON value exceeds bound")
                self.buffer = self.buffer[end:]
                return value
            except json.JSONDecodeError:
                scale.require(not self.eof, "Malformed native fixture JSON")
                scale.require(len(self.buffer.encode("utf-8")) <= limit,
                              "Native fixture JSON value exceeds bound")
                self.fill()

    def array(self):
        self.take("[")
        self.whitespace()
        count = 0
        if self.buffer.startswith("]"):
            self.take("]")
            return
        while True:
            count += 1
            scale.require(count <= MAX_ENVELOPES, "Too many fixture envelopes")
            yield self.value(MAX_ENVELOPE_BYTES * 2 + 4)
            self.whitespace()
            if self.buffer.startswith("]"):
                self.take("]")
                return
            self.take(",")


def parse_report(path, envelope):
    """Return small metadata; callback consumes each bounded envelope in order."""
    scale.require(path.stat().st_size <= MAX_REPORT_BYTES, "Native fixture report exceeds bound")
    result, seen = {}, set()
    with path.open(encoding="utf-8") as stream:
        reader = ReportReader(stream)
        reader.take("{")
        while True:
            key = reader.value(1024)
            scale.require(isinstance(key, str) and key not in seen, "Duplicate or invalid report key")
            seen.add(key)
            reader.take(":")
            if key in ARRAYS:
                count = 0
                for count, value in enumerate(reader.array(), 1):
                    envelope(key, count - 1, checked_hex(value, MAX_ENVELOPE_BYTES))
                scale.require(count > 0, "Incomplete fixture work")
            elif key in ("terminalEnvelope", "policyScale"):
                limit = artifact_limit(key)
                data = checked_hex(reader.value(limit * 2 + 4), limit)
                envelope(key, 0, data)
            else:
                result[key] = reader.value()
            reader.whitespace()
            if reader.buffer.startswith("}"):
                reader.take("}")
                break
            reader.take(",")
        reader.whitespace()
        scale.require(reader.eof and not reader.buffer, "Trailing native fixture JSON")
    scale.require(SPECIAL <= seen, "Incomplete native fixture report")
    scale.require(result.get("schema") == "sora-nexus-storage-work-v1" and
                  result.get("productionReady") is False and result.get("storageEngineRehearsed") is True,
                  "Completed diagnostic native storage composition required")
    return result


def checked_hex(value, limit):
    scale.require(isinstance(value, str) and value.startswith("0x") and
                  0 < len(value) - 2 <= limit * 2 and (len(value) - 2) % 2 == 0,
                  "Invalid bounded fixture hex")
    data = scale.raw(value)
    scale.require(value == "0x" + data.hex(), "Noncanonical fixture hex")
    return data


def digest_file(path, limit):
    digest, size = hashlib.sha256(), 0
    with path.open("rb") as stream:
        while data := stream.read(READ_BYTES):
            size += len(data)
            scale.require(size <= limit, "Fixture artifact exceeds bound")
            digest.update(data)
    return {"bytes": size, "sha256": digest.hexdigest()}


def reference(path, root, limit):
    return {"file": path.relative_to(root).as_posix(), **digest_file(path, limit)}


def checked_reference(root, row, limit, *, content=False):
    scale.require(isinstance(row, dict) and set(row) == {"file", "bytes", "sha256"},
                  "Unexpected fixture artifact reference")
    name = row["file"]
    scale.require(isinstance(name, str) and name and "\\" not in name, "Invalid fixture artifact path")
    relative = PurePosixPath(name)
    scale.require(not relative.is_absolute() and all(p not in ("..", ".") for p in relative.parts)
                  and relative.as_posix() == name, "Invalid fixture artifact path")
    path = root / name
    scale.require(path.resolve().is_relative_to(root.resolve()) and path.is_file() and not path.is_symlink(),
                  "Fixture artifact escapes bundle or is missing")
    scale.require(type(row["bytes"]) is int and 0 < row["bytes"] <= limit and
                  path.stat().st_size == row["bytes"], "Fixture artifact size mismatch")
    if content:
        with path.open("rb") as stream:
            data = stream.read(limit + 1)
        scale.require(len(data) == row["bytes"] and hashlib.sha256(data).hexdigest() == row["sha256"],
                      "Fixture artifact digest mismatch")
        return data
    scale.require(digest_file(path, limit) == {k: row[k] for k in ("bytes", "sha256")},
                  "Fixture artifact digest mismatch")
    return path


class EnvelopeFiles:
    """Re-iterable sequence; even after validation, each read checks its hash."""

    def __init__(self, root, rows):
        self.root, self.rows = root, rows

    def __len__(self):
        return len(self.rows)

    def __iter__(self):
        for row in self.rows:
            data = checked_reference(self.root, row, MAX_ENVELOPE_BYTES, content=True)
            yield "0x" + data.hex()


def pack(source, destination):
    source, destination = source.resolve(), destination.resolve()
    scale.require(not destination.exists(), "Fixture transport destination must be new")
    scale.require(source.is_file() and source.stat().st_size <= MAX_REPORT_BYTES,
                  "Native fixture report exceeds bound or is missing")
    destination.mkdir(parents=True)
    report = destination / "native-report.json"
    with source.open("rb") as incoming, report.open("xb") as outgoing:
        size = 0
        while data := incoming.read(READ_BYTES):
            size += len(data)
            scale.require(size <= MAX_REPORT_BYTES, "Native fixture report exceeds bound")
            outgoing.write(data)
    rows = {key: [] for key in ARRAYS}

    def write(key, index, data):
        name = f"{key}/{index:05}.scale" if key in ARRAYS else f"{key}.scale"
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as stream:
            stream.write(data)
        ref = reference(path, destination, artifact_limit(key))
        if key in ARRAYS:
            rows[key].append(ref)
        else:
            rows[key] = ref

    metadata = parse_report(report, write)
    identity = release.policy_identity((destination / rows["policyScale"]["file"]).read_bytes())
    scale.require("0x" + identity["commitment"] == metadata["policyCommitment"], "Policy commitment mismatch")
    index = {"schema": SCHEMA, "fixtureOnly": True, "productionReady": False,
             "victimAuthorizationVerified": False, "exactWasmRehearsed": False,
             "warning": "PUBLIC DIAGNOSTIC FIXTURE ONLY — NEVER DEPLOY",
             "nativeReport": reference(report, destination, MAX_REPORT_BYTES),
             "metadata": metadata, **rows}
    path = destination / "public-work-index.json"
    with path.open("x") as stream:
        json.dump(index, stream, indent=2)
        stream.write("\n")
    scale.require(path.stat().st_size <= MAX_FIELD_BYTES, "Fixture index exceeds bound")
    return {"schema": SCHEMA, "fixtureOnly": True, "productionReady": False,
            "index": str(path), "indexSha256": digest_file(path, MAX_FIELD_BYTES)["sha256"],
            "nativeReport": index["nativeReport"], "applyEnvelopes": len(rows["applyEnvelopes"]),
            "auditEnvelopes": len(rows["auditEnvelopes"]), "policyCommitment": metadata["policyCommitment"]}


def load(path, document):
    expected = {"schema", "fixtureOnly", "productionReady", "victimAuthorizationVerified",
                "exactWasmRehearsed", "warning", "nativeReport", "metadata", *SPECIAL}
    scale.require(set(document) == expected and document.get("schema") == SCHEMA and
                  document.get("fixtureOnly") is True and document.get("productionReady") is False and
                  document.get("victimAuthorizationVerified") is False and document.get("exactWasmRehearsed") is False,
                  "Explicit public fixture transport required")
    root = path.resolve().parent
    report = checked_reference(root, document["nativeReport"], MAX_REPORT_BYTES)
    for key in ARRAYS:
        scale.require(isinstance(document[key], list) and 0 < len(document[key]) <= MAX_ENVELOPES,
                      "Invalid fixture envelope count")
    counts, small = {key: 0 for key in ARRAYS}, {}

    def compare(key, index, expected_data):
        if key in ARRAYS:
            scale.require(index < len(document[key]), "Missing indexed fixture envelope")
            row = document[key][index]
            counts[key] += 1
        else:
            row = document[key]
        data = checked_reference(root, row, artifact_limit(key), content=True)
        scale.require(data == expected_data, "Fixture shard differs from native report")
        if key not in ARRAYS:
            small[key] = "0x" + data.hex()

    metadata = parse_report(report, compare)
    scale.require(json.dumps(metadata, sort_keys=True) == json.dumps(document["metadata"], sort_keys=True),
                  "Fixture metadata differs from native report")
    scale.require(all(counts[k] == len(document[k]) for k in ARRAYS), "Extra indexed fixture envelope")
    policy = scale.raw(small["policyScale"])
    scale.require("0x" + release.policy_identity(policy)["commitment"] == metadata["policyCommitment"],
                  "Policy commitment mismatch")
    return {"schema": "sora-nexus-PUBLIC-FIXTURE-ONLY-storage-v1", "fixtureOnly": True,
            "productionReady": False, "victimAuthorizationVerified": False, "exactWasmRehearsed": False,
            "transportSchema": SCHEMA, "compiledWorkSha256": document["nativeReport"]["sha256"],
            "work": {**metadata, **small, **{k: EnvelopeFiles(root, document[k]) for k in ARRAYS}}}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(pack(args.source, args.destination), indent=2))
