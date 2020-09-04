#!/bin/sh

socat - UNIX-LISTEN:.inside_docker_jobs/locked/socket <<EOF &
. /repos/parachain/scripts/partial/helpers.sh || exit 1
must git clone /repos/parachain
must mv parachain/* .
must mv parachain/.[a-hj-z]* .
must git checkout $1
ls -la ./scripts/partial/
exec bash ./scripts/localtestnet.sh
EOF
docker-compose up

