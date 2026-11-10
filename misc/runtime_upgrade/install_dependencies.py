#!/usr/bin/env python3
"""Install reviewed dependencies, resolving local source paths from this file.

Use the Python interpreter from a fresh virtual environment. Native wheels are
built with each source package's pinned Maturin backend and locked Rust graph.
"""

import argparse
import os
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wheel-dir", type=Path, help="build a wheelhouse instead of installing")
    args = parser.parse_args()
    tool_dir = Path(__file__).resolve().parent
    command = [sys.executable, "-m", "pip"]
    if args.wheel_dir:
        command += ["wheel", "--wheel-dir", str(args.wheel_dir.resolve())]
    else:
        command += ["install"]
    command += ["--requirement", str(tool_dir / "requirements.txt")]
    # Maturin's pyproject `locked = true` is read after its initial Cargo
    # metadata request. The PEP 517 argument must reach that earlier request
    # too, otherwise pip could regenerate a stale lock before building.
    environment = os.environ.copy()
    environment["MATURIN_PEP517_ARGS"] = "--locked"
    subprocess.run(command, cwd=tool_dir, env=environment, check=True)


if __name__ == "__main__":
    main()
