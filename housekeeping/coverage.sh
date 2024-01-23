#!/bin/sh
set -e

echo '⚡️ Running coverage'
grcov . --binary-path ./target/debug -s . -t cobertura --branch -o cobertura.xml --ignore-not-existing --ignore  "/opt/cargo/**" "target/debug" "node/src" "node/src" --log-level "ERROR" --llvm-path /usr/lib/llvm-14/bin

find . -type f -name '*.profraw' -delete