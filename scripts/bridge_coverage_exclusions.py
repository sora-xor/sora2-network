"""Identify only unreachable FRAME empty-Call stubs using LLVM function regions.

FRAME generates instance dispatch methods even for pallets with no dispatchables.
Their sole variant contains Never, so these methods cannot be called safely. LLVM
attributes their synthetic bodies to unrelated pallet declarations. Keep every
line also attributed to a real function, including real functions with zero hits.
"""

import json
from pathlib import Path
import re


EMPTY_CALL_FILES = {
    Path("vendor/sora2-common/pallets/channel/src/outbound/mod.rs"),
    Path("vendor/sora2-common/pallets/leaf-provider/src/lib.rs"),
}
INSTANCE_METHODS = (
    "get_dispatch_info", "is_feeless", "get_call_name", "get_call_index",
    "dispatch_bypass_filter", "authorize", "weight_of_authorize",
)


def function_records(reports):
    for report in reports:
        document = json.loads(report.read_text())
        if document.get("type") != "llvm.coverage.json.export":
            raise ValueError(f"Not an LLVM coverage export: {report}")
        for data in document["data"]:
            if "functions" not in data:
                raise ValueError(f"Function provenance is missing from {report}")
            yield from data["functions"]


def synthetic_empty_call(function):
    # Rust v0 encodes identifiers as <length><name>. Exact lengths distinguish
    # get_call_name from the callable static get_call_names, which remains counted.
    name = function["name"]
    return name.startswith("_R") and "4Call" in name and any(
        f"{len(method)}{method}" in name for method in INSTANCE_METHODS
    )


def remove_empty_call_lines(files, functions, root):
    """Mutate LCOV line maps; return an auditable list of removed lines/symbols."""
    generated = {path: set() for path in EMPTY_CALL_FILES}
    retained = {path: set() for path in EMPTY_CALL_FILES}
    symbols = {path: set() for path in EMPTY_CALL_FILES}
    for path in EMPTY_CALL_FILES & files.keys():
        source = (root / path).read_text()
        if re.search(r"#\s*\[\s*pallet\s*::\s*call\s*[\](]", source):
            raise ValueError(f"Empty-Call exclusion is invalid: {path} has dispatchables")
        if any(re.search(rf"\bfn\s+{method}\b", source) for method in INSTANCE_METHODS):
            raise ValueError(f"Empty-Call exclusion overlaps handwritten methods in {path}")

    for function in functions:
        synthetic = synthetic_empty_call(function)
        for region in function["regions"]:
            start, _, end, end_column, count, file_id, _, kind = region
            if kind != 0:  # Only LLVM CodeRegions, never gaps, skips, or expansions.
                continue
            path = Path(function["filenames"][file_id])
            if path.is_absolute():
                try:
                    path = path.relative_to(root)
                except ValueError:
                    continue
            if path not in EMPTY_CALL_FILES or path not in files:
                continue
            if synthetic:
                if function["count"] != 0 or count != 0:
                    raise ValueError(f"Unexpected executed empty-Call instance method in {path}")
                symbols[path].add(function["name"])
            # LLVM source intervals have an exclusive end column.
            last = end - (end_column == 1 and end > start)
            target = generated if synthetic else retained
            target[path].update(range(start, last + 1))

    removed = {}
    for path in EMPTY_CALL_FILES & files.keys():
        only_generated = generated[path] - retained[path]
        lines = sorted(line for line in only_generated if files[path].get(line) == 0)
        if lines:
            for line in lines:
                del files[path][line]
            removed[str(path)] = {"lines": lines, "functions": sorted(symbols[path])}
    return removed
