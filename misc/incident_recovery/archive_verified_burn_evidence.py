"""Fixed archive graph for freshly verified Burn pages and executing metadata.

Archive integrity alone never verifies a proof or clears a release gate. This
adapter closes the fresh verifier output graph without relaxing artifact limits.
"""
from __future__ import annotations

import gzip
import io
import json
import sys
from pathlib import Path

import archive_burn_evidence as archive_tools
import hashi_codec as scale

SCHEMA = "sora-nexus-verified-burn-evidence-archive-v1"
MANIFEST_NAME = "verification-complete.json"
INDEX_NAME = "verified-burn-evidence-archive.json"
VERIFICATION_REQUIRED = "Inspect the reconstructed derived pages against the bound fresh verifier receipt; rerun proof verification from the separately retained raw history"


def manifest_refs(db, data):
    report = json.loads(data)
    scale.require(report.get("schema") == "sora-nexus-verified-burn-stream-v1" and
                  report.get("checkpointHash") == scale.CHECKPOINT_HASH and
                  type(report.get("first")) is int and 0 < report["first"] <= report.get("last", 0) == scale.CHECKPOINT,
                  "Wrong verified Burn stream identity")
    scale.require(isinstance(report.get("pages"), list) and 0 < len(report["pages"]) <= 4_000_000 and
                  isinstance(report.get("metadata"), list) and 0 < len(report["metadata"]) <= 4096,
                  "Verified Burn graph bounds")
    seen = {MANIFEST_NAME}
    for item in report["pages"]:
        scale.require(set(item) == {"file", "sha256"} and item["file"] not in seen, "Duplicate/malformed verified page")
        seen.add(item["file"])
        archive_tools.expected(db, item["file"], item["sha256"], "page")
    code_hashes = set()
    for witness in report["metadata"]:
        scale.require(set(witness) == {"codeHash", "metadata", "specVersion"}, "Malformed executing metadata witness")
        code_hash = scale.raw(witness["codeHash"], 32)
        scale.integer(witness["specVersion"])
        scale.require(code_hash not in code_hashes, "Duplicate executing code witness")
        code_hashes.add(code_hash)
        item = witness["metadata"]
        scale.require(set(item) == {"file", "sha256"} and item["file"] not in seen, "Duplicate/malformed executing metadata")
        # The native verifier names metadata by content hash. Distinct executing
        # code hashes can produce identical metadata and share this exact file.
        # `expected` rejects a reused path with contradictory bytes or kind;
        # `seen` here retains only the manifest/page paths, which cannot alias.
        archive_tools.expected(db, item["file"], item["sha256"], "executing-metadata")
    return report


def metadata_refs(db, kind, data):
    del db
    scale.require(kind == "page", "Unknown verified Burn graph member")
    with gzip.GzipFile(fileobj=io.BytesIO(data)) as stream:
        decoded = stream.read(archive_tools.MAX_PAGE + 1)
    scale.require(len(decoded) <= archive_tools.MAX_PAGE, "Verified Burn page decompression bound")
    page = json.loads(decoded)
    scale.require(page.get("schema") == "sora-nexus-verified-burn-page-v1" and
                  isinstance(page.get("records"), list) and 0 < len(page["records"]) <= 16,
                  "Malformed verified Burn page")


def archive(manifest, destination, shard_bytes=archive_tools.DEFAULT_SHARD):
    return archive_tools.archive(manifest, destination, shard_bytes, graph=sys.modules[__name__])


def verify(index):
    return archive_tools.verify(index, graph=sys.modules[__name__])


def restore(index, destination):
    return archive_tools.restore(index, destination, graph=sys.modules[__name__])


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--manifest", type=Path)
    mode.add_argument("--restore", type=Path)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(restore(args.restore, args.destination) if args.restore else archive(args.manifest, args.destination)))
