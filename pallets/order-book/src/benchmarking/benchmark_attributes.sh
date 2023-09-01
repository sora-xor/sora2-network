#!/bin/bash

if which gawk > /dev/null 2>&1; then
  awk="gawk"
else
  awk="awk"
fi

max_preset=8
repeat=5

# MacOS default getopt doesn't support long args,
# so installing gnu version should make it work.
#
# brew install gnu-getopt
getopt_code=$($awk -f ./misc/getopt.awk <<EOF
Usage: sh ./benchmark_attributes.sh [-p MAX_PRESETS -r REPEATS] args...
Run multiple variants of attribute benchmarks (order-book) storing the results in corresponding files.
  -h, --help                  Show usage message
usage
exit 0
  -r, --repeat [number]       Select how many repetitions of this benchmark should run from within the wasm. (default: $repeat)
  -p, --max-preset [number]   Maximum number of preset to run to avoid running too long. (default: $max_preset)
EOF
)
eval "$getopt_code"

for bench_name in a b c
do
  for i in $(seq 1 $max_preset)
  do
    instance_name=$bench_name$i
    command="./target/release/framenode benchmark pallet --chain=local  --execution=wasm --wasm-execution=compiled --pallet order-book --extrinsic \"$instance_name\" --repeat $repeat --output ./benches/$instance_name.rs $*"
    eval "$command"
  done
done
