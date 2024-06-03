#!/bin/bash

binary="./target/debug/framenode"

chain="local"

execution="--execution native"

keep_db=0

if which gawk > /dev/null 2>&1; then
	awk="gawk"
else
	awk="awk"
fi

# Program to preserve log colors
#
# sudo apt-get install expect-dev
# brew install expect
if which unbuffer > /dev/null 2>&1; then
	unbuffer="unbuffer"
else
	unbuffer=""
fi

# MacOS default getopt doesn't support long args,
# so installing gnu version should make it work.
#
# brew install gnu-getopt
getopt_code=`$awk -f ./misc/getopt.awk <<EOF
Usage: sh ./run_script.sh [OPTIONS]...
Run frame node based local test net
  -h, --help                         Show usage message
usage
exit 0
  -d, --duplicate-log-of-first-node  Duplicate log of first node to console
duplicate_log=1
  -w, --disable-offchain-workers     Disable offchain workers
offchain_flags="--offchain-worker Never"
  -r, --use-release-build            Use release build
binary="./target/release/framenode"
  -s, --staging                      Using staging chain spec
chain="staging"
  -f, --fork                         Use fork chain spec
chain="fork.json"
  -e, --execution-wasm               Use wasm runtime
execution="--execution wasm --wasm-execution compiled"
  -k, --keep-db                      Keep previous chain state
keep_db=1
EOF
`
eval "$getopt_code"

#export RUST_LOG="beefy=info,ethereum_light_client=debug,bridge_channel=debug,dispatch=debug,eth_app=debug"
export RUST_LOG="info,runtime=debug"

localid=`mktemp`
tmpdir=`dirname $localid`

if [ ! -f $binary ]; then
	echo "Please build framenode binary"
	echo "for example by running command: cargo build --debug or cargo build --release"
	exit 1
fi

function local_id() {
  $awk "
    BEGIN { a=1 }
    /Local node identity is: /{
      if (a) {
        print \$11 > \"$localid\";
        fflush();
        a=0
      }
    }
    { print \"LOG: \" \$0; fflush() }
  "
}

function logger_for_first_node() {
	tee $1
}
if [ $keep_db -eq 0 ]; then
  find . -name "db*" -type d -maxdepth 1 -exec rm -rf {}/chains/sora-substrate-local/network {}/chains/sora-substrate-local/db \;
fi

port="10000"
wsport="9944"
num="0"
for name in alice bob charlie dave eve ferdie
do
	newport=`expr $port + 1`
	rpcport=`expr $wsport + 10`
	$binary key insert --chain $chain --suri "//${name}" --scheme ecdsa --key-type ethb --base-path db$num
	mkdir -p "db$num/chains/sora-substrate-$chain/bridge"
	cp misc/eth.json "db$num/chains/sora-substrate-$chain/bridge"
	if [ "$num" == "0" ]; then
		sh -c "$unbuffer $binary --pruning=archive --enable-offchain-indexing true $offchain_flags -d db$num --$name --port $newport --rpc-port $rpcport --chain $chain $execution 2>&1" | logger_for_first_node $tmpdir/port_${newport}_name_$name.txt &
	else
		sh -c "$binary --pruning=archive --enable-offchain-indexing true $offchain_flags -d db$num --$name --port $newport --rpc-port $rpcport --chain $chain $execution 2>&1" > $tmpdir/port_${newport}_name_$name.txt &
	fi
	echo SCRIPT: "Port:" $newport "P2P port:" $port "Name:" $name "WS:" $wsport "RPC:" $rpcport $tmpdir/port_${newport}_name_$name.txt
	port="$newport"
	wsport=`expr $wsport + 1`
	num=$(($num + 1))
done

wait

echo "SCRIPT: you can stop script by control-C hot key"
echo "SCRIPT: maybe framenode processes is still running, you can check it and finish it by hand"
echo "SCRIPT: in future this can be done automatically"

sleep 999999
