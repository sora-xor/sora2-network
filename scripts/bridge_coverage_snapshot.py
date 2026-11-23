"""Record the exact bridge source and manifest inputs used by a coverage run."""

import hashlib
import json
from pathlib import Path
import sys

from bridge_coverage import COMPONENTS, ROOT


def snapshot():
    paths = {ROOT / name for name in ["Cargo.toml", "Cargo.lock", "vendor/sora2-common/Cargo.toml", "vendor/sora2-common/Cargo.lock", "rust-toolchain.toml"]}
    # These local dependencies define the runtime and balance/permission behavior
    # exercised by the bridge suites, including the shared test configuration.
    source_trees = set(COMPONENTS.values()) | {
        "common/src", "pallets/assets/src", "pallets/technical/src", "pallets/permissions/src",
    }
    for source in source_trees:
        directory = ROOT / source
        paths.add(directory.parent / "Cargo.toml")
        paths.update(path for path in directory.rglob("*") if path.is_file())
    for config in [ROOT / ".cargo", ROOT / "vendor/sora2-common/.cargo"]:
        paths.update(path for path in config.rglob("*") if path.is_file())
    paths.update(ROOT / "scripts" / name for name in ["bridge_coverage.py", "bridge_coverage.sh", "bridge_coverage_exclusions.py", "bridge_coverage_snapshot.py", "test_bridge_coverage.py", "with_llvm_env.sh"])
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(paths) if path.is_file()}


def main():
    destination = Path(sys.argv[1])
    current = snapshot()
    destination.write_text(json.dumps(current, indent=2) + "\n")
    if len(sys.argv) == 3:
        before = json.loads(Path(sys.argv[2]).read_text())
        changed = sorted(path for path in current.keys() | before.keys() if current.get(path) != before.get(path))
        if changed:
            print("Coverage inputs changed during the run; rerun before accepting this report:", file=sys.stderr)
            print("\n".join(changed), file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
