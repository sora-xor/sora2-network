#!/bin/sh
set -e

printf '⚡️ Running coverage %s\n'
grcov . --binary-path ./target/debug -s . -t cobertura --branch -o cobertura_report.xml --ignore-not-existing --ignore  "/opt/cargo/**" "target/debug" "node/src" "node/src" --log-level "DEBUG" --llvm-path /usr/lib/llvm-14/bin
find . -type f -name '*.profraw' -delete