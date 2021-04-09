#!/bin/bash
set -ex

cmd0="cargo build --release"
cmd="${cmd0} --features"
declare -a nets=(
    dev-net
    test-net
    stage-net
    private-net
    coded-nets
    "dev-net coded-nets"
    "test-net coded-nets"
    "stage-net coded-nets"
    "private-net coded-nets"
)

$cmd0
for net in "${nets[@]}";
do
    $cmd "$net"
done