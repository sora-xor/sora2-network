#!/bin/sh

socat - UNIX-LISTEN:.inside_docker_jobs/locked/socket <<EOF &
. /repos/parachain/scripts/partial/helpers.sh || exit 1
must git clone /repos/parachain
must mv parachain/* .
must mv parachain/.[a-hj-z]* .
must git checkout $1
must mkdir -p /cache/target_current
must ln -s /cache/target_current ./target
exec bash ./scripts/localtestnet.sh
EOF
docker-compose up

