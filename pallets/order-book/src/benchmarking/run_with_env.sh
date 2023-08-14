#!/bin/bash

if which gawk > /dev/null 2>&1; then
	awk="gawk"
else
	awk="awk"
fi

# MacOS default getopt doesn't support long args,
# so installing gnu version should make it work.
#
# brew install gnu-getopt
getopt_code=$($awk -f ./misc/getopt.awk <<EOF
Usage: sh ./run_with_env.sh [OPTIONS] command
Run command
  -h, --help                Show usage message
usage
exit 0
  -f, --env-file [file]     File to load env from
EOF
)
eval "$getopt_code"

# shellcheck disable=SC2154
# shellcheck disable=SC1090
source "$env_file"
echo "$@"
