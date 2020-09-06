#!/bin/sh
. `dirname $0`/partial/helpers.sh || exit 1

must pwd_is_repos_topdir
jobsdir=".inside_docker_jobs"
lockdir="$jobsdir"
socket="$lockdir/socket"
tmpdir=`mktemp -d`

commit=$1
logfile=$2

shift
shift

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
exec bash ./scripts/localtestnet.sh --logdir-pattern "/cache/logs-$commit-XXXX" $@
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
