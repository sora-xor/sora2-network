#!/usr/bin/env bash
. `dirname $0`/partial/helpers.sh || exit 1

must pwd_is_repos_topdir
must command_exist socat

jobsdir=".inside_docker_jobs"
lockdir="$jobsdir"
socket="$lockdir/socket"
tmpdir=`mktemp -d`

getopt_code=`awk -f ./misc/getopt.awk <<EOF
Usage: ./scripts/docker_compose_up.sh [OPTIONS]...
Run local test net, downloading and (re)building on demand
  -h, --help                     Show usage message
usage
exit 0
  -r, --run [command]     Run this command inside docker
  -w, --with-last-commit  Use last commit
commit=\`get_current_commit\`
  -c, --commit [hex]      Use this commit
  -l, --logfile [path]    Use this logfile
EOF
`
eval "$getopt_code"

if [ "$logfile" == "" ]; then
	logfile=`mktemp -u`.log
	info "logfile is $logfile"
fi

docker_pid=$logfile.pid
flock_pid=""

function finalize() {
	if [ -f $docker_pid ]; then
		kill `cat $docker_pid 2> /dev/null` > /dev/null 2>&1
	fi
	if [ "$flock_pid" != "" ]; then
		kill -KILL $flock_pid > /dev/null 2>&1
	fi
}

function check_success() {
	tail -f $logfile.tmp | \
		awk "BEGIN { code=1 }
		     /SCRIPT IS FINISHED SUCCESEFULLY/{ system(\"mv $logfile.tmp $logfile\"); code=0; exit 0 }
		     /RUN COMMANDS IS FINISHED WITH CODE 0/{ code=0; exit 0 }
		     /RUN COMMANDS IS FINISHED WITH CODE [^0]/{ exit 1 }
		     { print \$0 }
		     END { exit code }"
}

trap finalize 2

if [ -f $docker_pid ]; then
	kill -0 `cat $docker_pid 2> /dev/null` > /dev/null 2>&1
	if [ $? == 0 ]; then
		info "process is already running"
		check_success
	fi
fi

if [ -f $logfile ]; then
	exec less -R $logfile
fi

must cat > $tmpdir/git_up.sh <<EOF
. /repos/parachain/scripts/partial/helpers.sh || exit 1
must git clone /repos/parachain
must mv parachain/* .
must mv parachain/.[a-hj-z]* .
must git checkout $commit
must mkdir -p /cache/target_current
must ln -s /cache/target_current ./target
if [ "$run" == "" ]; then
	exec bash ./scripts/localtestnet.sh --logdir-pattern "/cache/logs-$commit-XXXX" $@
else
	. ./nix-env.sh
	nix-shell --run "$run"
fi
EOF

must cat > $tmpdir/docker_up.sh <<EOF
docker-compose up > $logfile.tmp 2>&1 &
echo \$! > $docker_pid
EOF

must flock $lockdir -c "
echo LOCKING JOBS DIR
rm -f $socket
socat - UNIX-LISTEN:$socket < $tmpdir/git_up.sh &
pid=\$!
sh $tmpdir/docker_up.sh &
echo PASSING SCRIPT TO SOCKET
wait \$pid
echo UNLOCKING JOBS DIR
" &
flock_pid=$!
wait

check_success
