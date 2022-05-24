#!/bin/bash -v 

rm -rf db*
cargo b --features private-net --release --bin framenode
./run_script.sh

