#!/bin/sh
set -e

printf '⚡️ Running coverage %s\n'
# coverage and generate report
grcov . --binary-path ./target/debug -s . -t lcov --branch -o ./lcov_report --ignore-not-existing --ignore  "/opt/cargo/**" "target/debug" "node/src" --log-level="ERROR" --llvm-path='/usr/lib/llvm-14/bin'

# delete generated *.profraw
find . -type f -name '*.profraw' -delete