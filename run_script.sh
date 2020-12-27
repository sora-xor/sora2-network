#!/bin/sh

getopt_code=`awk -f ./misc/getopt.awk <<EOF
Usage: sh ./run_script.sh [OPTIONS]...
Run frame node based local test net
  -h, --help                         Show usage message
usage
exit 0
  -d, --duplicate-log-of-first-node  Duplicate log of first node to console
duplicate_log=1
EOF
`
eval "$getopt_code"




export RUST_LOG="sc_rpc=trace"

localid=`mktemp`
tmpdir=`dirname $localid`

if which gawk > /dev/null 2>&1; then
	awk="gawk"
else
	awk="awk"
fi

if [ ! -f ./target/release/framenode ]; then
	echo "Please build framenode binary"
	echo "for example by running command: cargo build --release"
	exit 1
fi

function local_id() {
  $awk "
    BEGIN { a=1 }
    /Local node identity is: /{
      if (a) {
        print \$10 > \"$localid\";
        fflush();
        a=0
      }
    }
    { print \"LOG: \" \$0; fflush() }
  "
}

function logger_for_first_node() {
	if [ "$duplicate_log" == "1" ]; then
		tee $1
	else
		cat > $1
	fi
}

port="10000"
wsport="9944"
start1="1"
for name in alice bob charlie dave eve
do
	newport=`expr $port + 1`
	if [ "$start1" == "1" ]; then
		sh -c "./target/release/framenode --tmp --$name --port $newport --ws-port $wsport --chain local 2>&1" | local_id | logger_for_first_node $tmpdir/port_${newport}_name_$name.txt &
	else
		sh -c "./target/release/framenode --tmp --$name --port $newport --ws-port $wsport --chain local --bootnodes /ip4/127.0.0.1/tcp/$port/p2p/`cat $localid` 2>&1" | local_id > $tmpdir/port_${newport}_name_$name.txt &
	fi
	echo SCRIPT: $newport $port $name $wsport $tmpdir/port_${newport}_name_$name.txt
	sleep 5
	port="$newport"
	wsport=`expr $wsport + 1`
	start1="0"
done

wait

echo SCRIPT: you can stop script by control-C hot key
echo SCRIPT: maybe framenode processes is still runnning, you can check it and finish it by hand
echo SCRIPT: in future this can be done automatically

sleep 999999

