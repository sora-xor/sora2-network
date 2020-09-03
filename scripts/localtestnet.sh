#!/bin/sh

# Constants
test_names="alice bob"
polkadot_commit="fd4b176f"
polkadot_repository="https://github.com/paritytech/polkadot"

# Deciding of fundamental variables
realpath=`realpath $0`
basename=`basename $realpath`
dirname=`dirname $realpath`
top=`realpath $dirname/..`
scripts="$top/scripts"
dir="$top/tmp"

# Quick check for correctness of this variables
test -f $scripts/localtestnet.sh || exit 1
test -f $top/Cargo.toml          || exit 1
test -f $top/node/Cargo.toml     || exit 1
test -f $top/runtime/Cargo.toml  || exit 1

function check_polkadot_binary() {
	if [ "$polkadot" == "" ]; then
		which $1 > /dev/null 2>&1 && $1 --help | head -n 1 | grep -q $polkadot_commit
		if [ $? == 0 ]; then
			polkadot=`which $1`
		else
			false
		fi
	fi
}

function build_polkadot_on_demand() {
	polkadot_ready="$dir/polkadot_ready"
	polkadot_path="$polkadot_ready/target/release"
	polkadot_binary="$polkadot_path/polkadot"
	echo "SCRIPT: Polkadot binary of $polkadot_commit commit build is not found in PATH, building it"
	if [ ! -d $dir/polkadot_ready  ]; then
		echo "SCRIPT: Polkadot is not cloned, cloning repository"
		mkdir -p $dir || exit 1
		pushd $dir
			git clone $polkadot_repository && \
				mv polkadot polkadot_ready || \
					exit 1
		popd
	fi
	if [ ! -f $polkadot_binary ]; then
		echo "SCRIPT: Polkadot binary is not builded, building it"
		pushd $polkadot_ready
			git checkout $polkadot_commit || exit 1
			ln -sf $top/misc/Makefile   . || exit 1
			ln -sf $top/misc/shell.nix  . || exit 1
			ln -sf $top/misc/nix-env.sh . || exit 1
			make cargo-build-release      || exit 1
		popd
	fi
	if [ -f $polkadot_binary ]; then
		echo "SCRIPT: Checking that polkadot binary can run and is correct"
		check_polkadot_binary $polkadot_binary
		if [ $? != 0 ]; then
			echo "SCRIPT: Polkadot binary is incorrect"
			exit 1
		fi
	else
		echo "SCRIPT: Polkadot binary it not exist in target/release folder, building is failed"
		exit 1
	fi
}

polkadot=""
check_polkadot_binary polkadot
check_polkadot_binary ../polkadot/target/release/polkadot
if [ $? == 0 ]; then
	echo "SCRIPT: Polkadot binary of $polkadot_commit commit is already exist, skiping build and use it"
else
	build_polkadot_on_demand
fi




exit

# Parameters of testnet
relaychain_nodes_count=2

parachains="200"
parachain_fullnodes_count=2
parachain_collators_count=4

dir="$PWD/tmp"
bin="$dir/bin"
polkadot="$dir/polkadot"
chain_json="$PWD/misc/rococo-custom.json"
parachain="$dir/parachain"
logdir_pattern="/tmp/rococo-localtestnet-logs-XXXXXXXX"

# Empty values
relaychain_nodes=""
parachain_nodes=""
pids=""

function get_test_name() {
	echo $test_names | fmt -w 1 | awk "NR == `expr $1 + 1` { print \$0 }"
}

function check_dirs_and_files() {
	test -f $bin/polkadot-js-api || exit 1
	test -d $iroha || exit 1
	test -f $iroha/config.json || exit 1
	test -d $polkadot || exit 1
	test -f $chain_json || exit 1
}

function create_log_dir() {
	log=`mktemp -u $logdir_pattern`
	mkdir -p $log
	echo "Rococo localtestnet logdir is: $log"
}

function add_cargo_path() {
	PATH="$1/target/release:$PATH"
	export PATH
}

function add_path() {
	PATH="$1:$PATH"
	export PATH
}

function start_iroha_node() {
	prefix=$log/iroha_node_$1
	logfile=$prefix.log
	rm -Rf $iroha/blocks > /dev/null 2>&1
	mkdir -p $log/iroha
	cp $iroha/config.json $log/iroha/
	(sh -c "cd $log/iroha; exec iroha 2>&1" & echo $! >&3) 3>$prefix.pid | \
		awk "{ print \$0; fflush() }" > $logfile &
	pids="$pids `cat $prefix.pid`"
	echo "Iroha node $1 is running"
}

function start_relaychain_node() {
	wsport=`expr $1 + 9944`
	port=`expr $1 + 30333`
	test_name=`get_test_name $1`
	prefix=$log/relaychain_node_$1
	localid=$prefix.localid
	logfile=$prefix.log
	bootnodes=""
	if [ "$relay_nodes" != "" ]
	then
		bootnodes="--bootnodes $relaychain_nodes"
	fi
	(sh -c "exec polkadot \
		  --chain $chain_json \
	          --tmp \
	          --ws-port $wsport \
	          --port $port \
	          --$test_name \
		  $bootnodes 2>&1" & echo $! >&3) 3>$prefix.pid | \
	    awk "BEGIN { a=1 }
		 /Local node identity is: /{ if (a) {
		   print \$11 > \"$localid\"; fflush(); a=0 } }
		 { print \$0; fflush() }" > $logfile &
	pids="$pids `cat $prefix.pid`"
	while [ ! -f $localid ]
	do
		sleep 0.1
	done
	echo "Relaychain node $1 is running"
	relaychain_nodes="$relaychain_nodes /ip4/127.0.0.1/tcp/$port/p2p/`cat $localid`"
}

function start_parachain_fullnode() {
	wsport=`expr $1 + 19944 - $2`
	port=`expr $1 + 31333 - $2`
	test_name=`get_test_name $1`
	prefix=$log/parachain_$2_fullnode_$1
	localid=$prefix.localid
	logfile=$prefix.log
	relaychain_bootnodes=""
	if [ "$relaychain_nodes" != "" ]
	then
		relaychain_bootnodes="--bootnodes $relaychain_nodes"
	fi
	parachain_bootnodes=""
	if [ "$parachain_nodes" != "" ]
	then
		parachain_bootnodes="--bootnodes $parachain_nodes"
	fi
	(sh -c "parachain-collator \
		  --tmp \
		  `if [ $1 == 0 ]; then echo --offchain-worker Always; else echo --offchain-worker Never; fi` \
		  --alice \
		  --ws-port $wsport \
		  --port $port \
		  --parachain-id $2 \
		  $parachain_bootnodes \
		  -- --chain $chain_json \
	          $relaychain_bootnodes 2>&1" & echo $! >&3) 3>$prefix.pid | \
	    awk "BEGIN { a=1 }
		 /Local node identity is: /{ if (a) {
		   print \$11 > \"$localid\"; fflush(); a=0 } }
		 { print \$0; fflush() }" > $logfile &
	pids="$pids `cat $prefix.pid`"
	while [ ! -f $localid ]
	do
		sleep 0.1
	done
	echo "Parachain $2 fullnode $1 is running"
	parachain_nodes="$parachain_nodes /ip4/127.0.0.1/tcp/$port/p2p/`cat $localid`"
}

function start_parachain_collator() {
	wsport=`expr $1 + 29944 - $2`
	port=`expr $1 + 32333 - $2`
	test_name=`get_test_name $1`
	prefix=$log/parachain_$2_collator_$1
	localid=$prefix.localid
	logfile=$prefix.log
	relaychain_bootnodes=""
	if [ "$relaychain_nodes" != "" ]
	then
		relaychain_bootnodes="--bootnodes $relaychain_nodes"
	fi
	parachain_bootnodes=""
	if [ "$parachain_nodes" != "" ]
	then
		parachain_bootnodes="--bootnodes $parachain_nodes"
	fi
	(sh -c "parachain-collator \
		  --tmp \
		  --validator \
		  --alice \
		  --ws-port $wsport \
		  --port $port \
		  --parachain-id $2 \
		  $parachain_bootnodes \
		  -- --chain $chain_json \
	          $relaychain_bootnodes 2>&1" & echo $! >&3) 3>$prefix.pid | \
	    awk "BEGIN { a=1 }
		 /Local node identity is: /{ if (a) {
		   print \$11 > \"$localid\"; fflush(); a=0 } }
		 { print \$0; fflush() }" > $logfile &
	pids="$pids `cat $prefix.pid 2> /dev/null`"
	while [ ! -f $localid ]
	do
		sleep 0.1
	done
	echo "Parachain $2 collator $1 is running"
	parachain_nodes="$parachain_nodes /ip4/127.0.0.1/tcp/$port/p2p/`cat $localid`"
}

function waiting_for_ready_state() {
	peers=`expr $parachain_fullnodes_count + $parachain_collators_count - 1`
	while [ ! -f $log/ready.txt ]
	do
		cat $log/parachain_$1_fullnode_0.log | \
	    	 	awk -F "[#( ]" "
		 		/Parachain.*Idle.*peers.*best: / {
		 			if ((\$11 == $peers) && (\$15 >= 1)) {
						print \$0 > \"$log/ready.txt\"
						exit
		 			}
		 		}"
		sleep 0.1
	done
	echo "Ready for testing, parachain $1 blocks is produced"
}

function run_tests() {
	sh -c "cd $iroha; bridge-tester"
	#polkadot-js-api --seed "//Alice" tx.balances.transfer 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty 999
}

function finalize() {
	for pid in $pids
	do
		kill -KILL $pid > /dev/null 2>&1
	done
	# Remove log dir, comment or uncomment if needed
	#rm -Rf $log
	exit
}

trap finalize 0 1 2 3 6 15

check_dirs_and_files
create_log_dir

add_path $bin
add_cargo_path $iroha
add_cargo_path $polkadot
add_cargo_path $parachain

#export RUST_LOG="iroha_bridge=trace,sc_rpc=trace"
export RUST_LOG="sc_rpc=trace"

for iroha_node_number in `seq 1 $iroha_nodes_count`
do
	start_iroha_node `expr $iroha_node_number - 1`
done

for relaychain_node_number in `seq 1 $relaychain_nodes_count`
do
	start_relaychain_node `expr $relaychain_node_number - 1`
done

for parachain_id in $parachains
do

	for parachain_fullnode_number in `seq 1 $parachain_fullnodes_count`
	do
		start_parachain_fullnode `expr $parachain_fullnode_number - 1` $parachain_id
	done

	for parachain_collator_number in `seq 1 $parachain_collators_count`
	do
		start_parachain_collator `expr $parachain_collator_number - 1` $parachain_id
	done

	parachain-collator export-genesis-wasm > $log/parachain_${parachain_id}.wasm
	cat $log/parachain_${parachain_id}_collator_0.log | \
		awk "/Parachain genesis state: /{ print \$9; exit }" > $log/genesis_${parachain_id}.txt

	while true; do
		polkadot-js-api \
			--ws "ws://127.0.0.1:9944" \
			--sudo \
			--seed "//Alice" \
			tx.registrar.registerPara \
			$parachain_id \
			'{"scheduling":"Always"}' \
			@"$log/parachain_${parachain_id}.wasm" \
			"`cat $log/genesis_${parachain_id}.txt`" | \
	    	    grep -q '"InBlock": "0x' && break
    	done
	echo "Parachain $parachain_id is registred"

done

for parachain_id in $parachains
do
	waiting_for_ready_state $parachain_id
done
run_tests

wait


#npm install -g @polkadot/api-cli --prefix $top/local
#clone_and_build cargo polkadot https://github.com/paritytech/polkadot fd4b176f target/release/polkadot


