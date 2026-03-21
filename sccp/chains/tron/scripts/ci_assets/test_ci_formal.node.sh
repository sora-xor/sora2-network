#!/usr/bin/env bash
set -euo pipefail

echo "[sccp-ci-formal] check repository hygiene"
npm run check:repo-hygiene

echo "[sccp-ci-formal] compile + unit checks"
npm test

echo "[sccp-ci-formal] deployment script checks"
npm run test:deploy-scripts

echo "[sccp-ci-formal] formal-assisted checks"
npm run test:formal-assisted:ci

echo "[sccp-ci-formal] OK"
