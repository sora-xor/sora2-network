#!/usr/bin/env python3
"""Gate LLVM line coverage for every SORA Nexus bridge component separately."""

import argparse
import html
import json
from pathlib import Path
import sys

from bridge_coverage_exclusions import function_records, remove_empty_call_lines

ROOT = Path(__file__).resolve().parents[1]
COMPONENTS = {
    "eth-bridge": "pallets/eth-bridge/src",
    "bridge-proxy": "pallets/bridge-proxy/src",
    "bridge-multisig": "vendor/pallet-multisig/src",
    **{
        name: f"vendor/sora2-common/pallets/{directory}/src"
        for name, directory in [
            ("beefy-light-client", "beefy-light-client"),
            ("bridge-channel", "channel"),
            ("bridge-common", "bridge-common"),
            ("bridge-data-signer", "data-signer"),
            ("bridge-types", "types"),
            ("dispatch", "dispatch"),
            ("evm-fungible-app", "evm-fungible-app"),
            ("jetton-app", "jetton-app"),
            ("leaf-provider", "leaf-provider"),
            ("liberland-bridge-provider", "liberland-bridge-provider"),
            ("multisig-verifier", "multisig-verifier"),
            ("shared-multisig", "multisig"),
            ("parachain-bridge-app", "parachain-app"),
            ("substrate-bridge-app", "substrate-app"),
            ("substrate-bridge-channel", "substrate-channel"),
        ]
    },
}
EXCLUDED = {"test.rs", "tests.rs", "mock.rs", "fixtures.rs", "test_utils.rs",
            "benchmarking.rs", "weights.rs", "coverage.rs"}


def component(path):
    handwritten_weights = path == Path("vendor/sora2-common/pallets/beefy-light-client/src/weights.rs")
    if "tests" in path.parts or (path.name in EXCLUDED and not handwritten_weights) or path.name.endswith("_tests.rs"):
        return None
    for name, source in COMPONENTS.items():
        if path.is_relative_to(Path(source)):
            return name
    return None


def read_lcov(paths):
    files = {}
    for report in paths:
        current = None
        for line in report.read_text().splitlines():
            if line.startswith("SF:"):
                path = Path(line[3:])
                if path.is_absolute():
                    try:
                        path = path.relative_to(ROOT)
                    except ValueError:
                        current = None
                        continue
                current = files.setdefault(path, {}) if component(path) else None
            elif line.startswith("DA:") and current is not None:
                number, count, *_ = line[3:].split(",")
                number, count = int(number), int(count)
                current[number] = max(current.get(number, 0), count)
            elif line == "end_of_record":
                current = None
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lcov", nargs="+", type=Path)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/bridge-coverage-reports")
    parser.add_argument("--threshold", type=float, default=99)
    parser.add_argument("--no-gate", action="store_true", help="Inspect a partial baseline without enforcing the threshold")
    parser.add_argument("--llvm-json", nargs="+", type=Path, default=[],
                        help="Matching LLVM JSON exports with function regions for empty-Call provenance")
    args = parser.parse_args()
    files = read_lcov(args.lcov)
    synthetic_exclusions = remove_empty_call_lines(files, function_records(args.llvm_json), ROOT) if args.llvm_json else {}
    summary = {}
    details = {}
    for name in COMPONENTS:
        selected = {str(path): lines for path, lines in files.items() if component(path) == name}
        total = sum(len(lines) for lines in selected.values())
        covered = sum(sum(count > 0 for count in lines.values()) for lines in selected.values())
        percent = 100 * covered / total if total else 0
        summary[name] = {"covered": covered, "lines": total, "percent": percent,
                         "passed": total > 0 and percent > args.threshold}
        for path, lines in selected.items():
            details[path] = {"covered": sum(count > 0 for count in lines.values()),
                             "lines": len(lines), "uncovered": [n for n, count in sorted(lines.items()) if count == 0]}
        print(f"{name:28} {covered:5}/{total:<5} {percent:6.2f}%  {'PASS' if summary[name]['passed'] else 'FAIL'}")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    report = {"metric": "LLVM executable source lines", "threshold_exclusive": args.threshold,
              "components": summary, "files": details,
              "generated_empty_call_exclusions": synthetic_exclusions,
              "exclusions": "Test/fixture/mock modules, generated weights and benchmark harnesses. Handwritten BEEFY weights remain counted. Inline test modules use coverage(off)."}
    if args.llvm_json:
        report["exclusions"] += " LLVM function regions additionally exclude only unreachable generated empty-Call instance methods in channel outbound and leaf-provider; lines shared with any real or callable static function remain counted."
    (args.output_dir / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    rows = []
    for name, value in summary.items():
        rows.append(f"<tr><td>{name}</td><td>{value['covered']}/{value['lines']}</td><td>{value['percent']:.2f}%</td><td>{'PASS' if value['passed'] else 'FAIL'}</td></tr>")
    source_pages = []
    for path, lines in sorted(files.items()):
        text = (ROOT / path).read_text().splitlines()
        rendered = []
        for number, line in enumerate(text, 1):
            count = lines.get(number)
            css = "miss" if count == 0 else "hit" if count is not None else ""
            rendered.append(f'<span class="{css}"><a id="L{number}">{number:5}</a> {"" if count is None else str(count):>7} {html.escape(line)}</span>')
        page_name = str(path).replace("/", "__") + ".html"
        (args.output_dir / page_name).write_text("<meta charset=utf-8><style>span{display:block}.miss{background:#ffe0e0}.hit{background:#e4f5e4}pre{line-height:1.35}</style><h1>" + html.escape(str(path)) + "</h1><pre>" + "\n".join(rendered) + "</pre>")
        info = details[str(path)]
        pct = 100 * info["covered"] / info["lines"] if info["lines"] else 0
        source_pages.append(f'<li><a href="{page_name}">{path}</a>: {pct:.2f}% ({info["covered"]}/{info["lines"]})</li>')
    (args.output_dir / "index.html").write_text('<meta charset=utf-8><title>SORA Nexus bridge coverage</title><style>body{font:16px system-ui;max-width:1100px;margin:40px auto}td,th{text-align:left;padding:8px 20px;border-bottom:1px solid #ddd}</style><h1>SORA Nexus bridge coverage</h1><p>' + html.escape(report["exclusions"]) + f'</p><p>Each component must exceed {args.threshold:g}% line coverage.</p><table><tr><th>Component</th><th>Covered/lines</th><th>Coverage</th><th>Gate</th></tr>' + "".join(rows) + '</table><h2>Source files</h2><ul>' + "".join(source_pages) + '</ul>')
    return 0 if args.no_gate or all(v["passed"] for v in summary.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
