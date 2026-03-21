#!/usr/bin/env bash
set -euo pipefail

echo "[sccp-ci-formal] check repository hygiene"
bash ./scripts/check_repo_hygiene.sh

echo "[sccp-ci-formal] formal-assisted checks"
./scripts/test_formal_assisted.sh

echo "[sccp-ci-formal] deployment script checks"
./scripts/test_deploy_scripts.sh

echo "[sccp-ci-formal] OK"
