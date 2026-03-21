#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

python3 -B - <<'PY'
import ast
from pathlib import Path

ast.parse(Path("scripts/deploy_mainnet.py").read_text(encoding="utf-8"), filename="scripts/deploy_mainnet.py")
PY
python3 -B scripts/deploy_mainnet.py --help >/dev/null
