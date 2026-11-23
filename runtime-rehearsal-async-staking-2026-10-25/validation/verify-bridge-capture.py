#!/usr/bin/env python3
"""Verify extracted request values against the complete raw paginated capture.

The request prefix is derived from the source key/hash pairs. The separate
payload-recovery verifier checks that prefix against the runtime storage domain.
No RPC, signing, runtime execution, or operator readiness is performed here.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest_file(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def valid_hex(value):
    return isinstance(value, str) and re.fullmatch(r"0x(?:[0-9a-f]{2})+", value) is not None


def verify_capture(capture_path, source_path, pages_path):
    capture_path, source_path, pages_path = map(Path, (capture_path, source_path, pages_path))
    capture_bytes, source_bytes = capture_path.read_bytes(), source_path.read_bytes()
    capture, source = json.loads(capture_bytes), json.loads(source_bytes)
    require(capture.get("status") == "passed" and capture.get("requestScanComplete") is True,
            "Capture does not claim a completed request scan")
    page_size, cap, count = (capture.get(key) for key in ("pageSize", "requestsCap", "requestCount"))
    require(type(page_size) is int and page_size > 0, "Invalid page size")
    require(type(cap) is int and cap > 0, "Invalid request cap")
    require(type(count) is int and 0 <= count <= cap, "Invalid captured request count")
    require(isinstance(source, list) and source, "Extracted source rows are missing")
    wanted = {}
    prefix = None
    for row in source:
        key, call_hash, value = (row.get(field) for field in ("key", "hash", "rawValue"))
        require(valid_hex(key) and valid_hex(call_hash) and len(call_hash) == 66,
                "Invalid extracted key/hash encoding")
        require(len(key) > 66 and key.endswith(call_hash[2:]), "Extracted key/hash mismatch")
        current_prefix = key[:-64]
        if prefix is None:
            prefix = current_prefix
        require(prefix == current_prefix, "Extracted source uses multiple request prefixes")
        require(valid_hex(value), "Invalid extracted raw value")
        require(key not in wanted, "Duplicate extracted source key")
        wanted[key] = value

    remaining = dict(wanted)
    raw_hash, raw_bytes = hashlib.sha256(), 0
    observed, pages, previous, last_full_cursor = 0, 0, None, None
    terminal = False
    with gzip.open(pages_path, "rb") as stream:
        for line in stream:
            raw_hash.update(line)
            raw_bytes += len(line)
            require(not terminal, "Capture continues after terminal short page")
            page = json.loads(line)
            require(isinstance(page, dict), "Invalid captured page")
            require(page.get("startKey") == previous, "Captured page cursor mismatch")
            rows = page.get("rows")
            require(isinstance(rows, list), "Captured page rows are missing")
            requested = min(page_size, cap - observed)
            require(requested > 0, "Capture exhausted its cap without a terminal observation")
            require(len(rows) <= requested, "Captured page exceeds requested bound")
            for entry in rows:
                require(isinstance(entry, list) and len(entry) == 2, "Invalid raw captured row")
                key, value = entry
                require(valid_hex(key) and len(key) == len(prefix) + 64 and key.startswith(prefix),
                        "Captured key outside expected request prefix")
                require(previous is None or key > previous, "Duplicate or unordered captured key")
                require(valid_hex(value), "Invalid captured raw value")
                previous = key
                observed += 1
                if key in remaining:
                    require(value == remaining[key], "Extracted raw value differs from captured value")
                    del remaining[key]
            pages += 1
            if len(rows) < requested:
                terminal = True
            else:
                last_full_cursor = previous
    require(pages > 0 and terminal, "Capture has no short terminal page; it may be truncated")
    require(observed == count, "Captured request count differs from raw pages")
    require(capture.get("requestCursor") == last_full_cursor, "Capture final cursor differs from raw pagination")
    require(not remaining, "Extracted source key missing from captured pages")
    require(raw_bytes == capture.get("requestPagesBytes"), "Raw capture byte length differs")
    require(raw_hash.hexdigest() == capture.get("requestPagesSha256"), "Raw capture hash differs")
    return {
        "status": "passed", "localOnly": True, "remoteRequests": 0,
        "capturedRequests": observed, "pagesVerified": pages,
        "extractedRowsMatched": len(wanted), "terminalPageVerified": True,
        "requestPrefix": prefix, "rawPagesBytes": raw_bytes,
        "rawPagesSha256": raw_hash.hexdigest(),
        "sourceSha256": hashlib.sha256(source_bytes).hexdigest(),
        "gzipSha256": digest_file(pages_path),
        "captureSha256": hashlib.sha256(capture_bytes).hexdigest(),
        "helperSha256": digest_file(__file__),
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture")
    parser.add_argument("source")
    parser.add_argument("pages")
    parser.add_argument("--output")
    args = parser.parse_args()
    proof = verify_capture(args.capture, args.source, args.pages)
    text = json.dumps(proof, indent=2) + "\n"
    if args.output:
        Path(args.output).write_text(text)
    print(text, end="")
