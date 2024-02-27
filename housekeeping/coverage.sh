#!/bin/sh

grcov . --binary-path ./target/debug -s . -t cobertura --branch -o ./cobertura_report --ignore-not-existing --ignore  "/opt/cargo/**" "target/debug" "node/src"
find . -type f -name '*.profraw' -delete
