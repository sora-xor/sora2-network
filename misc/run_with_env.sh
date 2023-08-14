#!/bin/bash

if which gawk > /dev/null 2>&1; then
	awk="gawk"
else
	awk="awk"
fi

env_file=/dev/null

# MacOS default getopt doesn't support long args,
# so installing gnu version should make it work.
#
# brew install gnu-getopt
getopt_code=$($awk -f ./misc/getopt.awk <<EOF
Usage: sh ./run_with_env.sh [-f FILE] command
Run command with environmental variables defined in $(tput bold)file$(tput sgr0)
  -h, --help                Show usage message
usage
exit 0
  -f, --env-file [file]     File to load env from
EOF
)
eval "$getopt_code"

set -a
. "$env_file"
set +a
eval "$@"
