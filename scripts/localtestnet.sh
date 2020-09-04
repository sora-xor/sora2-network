#!/bin/sh
. `dirname $0`/partial/helpers.sh || exit 1

# Default configuration
#

remove_log_dir_on_finalize=1

relaychain_nodes_count=2

parachains="200"
parachain_fullnodes_count=2
parachain_collators_count=4

skip_build_of_parachain_binary_if_it_exist=1
enable_incremental_compilation=0
remove_binary_for_rebuild=0

export RUST_LOG="sc_rpc=trace"

# Preparing environment
#

# Constants
test_names="alice bob"
polkadot_commit="fd4b176f"
polkadot_repository="https://github.com/paritytech/polkadot"
logdir_pattern="/tmp/rococo-localtestnet-logs-XXXXXXXX"
cache="/tmp/parachain_cargo_target_build_cache"

# Deciding of fundamental variables
realpath=`realpath $0`
basename=`basename $realpath`
dirname=`dirname $realpath`
top=`realpath $dirname/..`
chain_json="$top/misc/rococo-custom.json"
scripts="$top/scripts"
dir="$top/tmp"

# Quick check for correctness of this variables
must [ -f $chain_json              ]
must [ -f $scripts/localtestnet.sh ]
must [ -f $top/Cargo.toml          ]
must [ -f $top/node/Cargo.toml     ]
must [ -f $top/runtime/Cargo.toml  ]

# Declare functions
#

function link_makefile_etc() {
	must ln -sf $top/misc/Makefile   .
	must ln -sf $top/misc/shell.nix  .
	must ln -sf $top/misc/nix-env.sh .
}

function check_polkadot_binary() {
	if [ "$polkadot" == "" ]; then
		command_exist $1 && $1 --help | expect $polkadot_commit
		if [ $? == 0 ]; then
			polkadot=`which $1`
		else
			false
		fi
	fi
}

function check_api_binary() {
	if [ "$api" == "" ]; then
		command_exist $1 && $1 --version | expect "[0-9]+\.[0-9]+\.[0-9]+"
		if [ $? == 0 ]; then
			api=`which $1`
		else
			false
		fi
	fi
}

function install_api_on_demand() {
       command_exist npm || \
               panic "npm is not found, please install npm"
       expected_api="$dir/local/bin/polkadot-js-api"
       if [ ! -f $expected_api ]; then
               info "polkadot-js-api command not found, installing it"
               must npm install -g @polkadot/api-cli --prefix "$dir/local"
       fi
       check_api_binary $expected_api || \
               panic "polkadot-js-api is not working"
}

function build_polkadot_on_demand() {
	polkadot_ready="$dir/polkadot_ready"
	polkadot_path="$polkadot_ready/target/release"
	polkadot_binary="$polkadot_path/polkadot"
	info "polkadot binary of $polkadot_commit commit build is not found, building it"
	if [ ! -d $dir/polkadot_ready  ]; then
		info "polkadot is not cloned, cloning repository"
		must mkdir -p $dir
		pushd $dir
			git clone $polkadot_repository && \
				mv polkadot polkadot_ready || \
					bomb 3 1 "$@"
		popd
	fi
	if [ ! -f $polkadot_binary ]; then
		info "polkadot binary is not builded, building it"
		pushd $polkadot_ready
			must git checkout $polkadot_commit
			link_makefile_etc
			must make cargo-build-release
		popd
	fi
	if [ -f $polkadot_binary ]; then
		info "checking that polkadot binary can run and is correct"
		check_polkadot_binary $polkadot_binary || \
			panic "polkadot binary is incorrect"
	else
		panic "polkadot binary it not exist in target/release folder, building is failed"
	fi
}

api=""
check_api_binary polkadot-js-api
check_api_binary $top/../rococo-localtestnet-scripts/local/bin/polkadot-js-api
on_success info "polkadot-js-api is already exist, skipping install and using it" \
	|| install_api_on_demand

polkadot=""
check_polkadot_binary polkadot
check_polkadot_binary $top/../polkadot/target/release/polkadot
on_success info "polkadot binary of $polkadot_commit commit is already exist, skiping build and using it" \
	|| build_polkadot_on_demand


#
# Compilation of parachain
#

function get_last_commit_in_cache() {
	get_all_commits | awk "{ if (system(\"test -f $cache\" $1 \".exist\")==0) { print $1; exit } }"
}

function check_parachain_binary_and_cache_target() {
	test -f $parachain \
		|| panic "parachain binary is not found after build"
	$parachain --version | expect "parachain-collator" \
		|| panic "parachain binary is incorrect"
	test $enable_incremental_compilation == 0 && return 0
	mkdir -p $cache
	path="$cache/`get_current_commit`"
	test -f $path.exist && return 0
	verbose tar -cf $path.tar.tmp target \
		|| panic "backuping of target dir to cache is failed"
	must mv $path.tar.tmp $path.fresh.tar
	must verbose rdiff signature $path.fresh.tar \
		             	     $path.tar.sig
	sha256sum            	     $path.tar.sig > \
	                     	     $path.tar.sig.sha256 || bomb 1 0 "$@"
	get_all_commits | head -n 1000 > $path.exist || bomb
}

function restore_target_from_cache_on_demand() {
	test $enable_incremental_compilation == 0 && return 0
	test -d target && return 0
	commit=`get_last_commit_in_cache`
	tarfile=`first $cache/${commit}*.tar`
	if [ -f $tarfile ]; then
		must tar -xf $cache/$commit.tar
	else
		unimplemented
	fi
}

function build_parachain_binary() {
	pushd $top
		link_makefile_etc
		restore_target_from_cache_on_demand
		if [ $remove_binary_for_rebuild == 1 ]; then
			rm -f $parachain > /dev/null 2>&1
		fi
		if [ ! -f $parachain -o $skip_build_of_parachain_binary_if_it_exist == 0 ]; then
			verbose make cargo-build-release
		fi
		check_parachain_binary_and_cache_target
	popd
}

parachain="$top/target/release/parachain-collator"
build_parachain_binary


#
# Declaration of functions required to make local testnet
#

# Empty values
relaychain_nodes=""
parachain_nodes=""
pids=""

function get_test_name() {
	echo $test_names | fmt -w 1 | awk "NR == `expr $1 + 1` { print \$0 }"
}

function check_dirs_and_files() {
	# Additional checks can be added here if needed
	return 0
}

function create_log_dir() {
	log=`mktemp -u $logdir_pattern` || bomb
	must mkdir -p $log
	info "rococo localtestnet logdir is: $log"
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
	(sh -c "exec $polkadot \
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
	info "relaychain node $1 is running"
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
	(sh -c "$parachain \
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
	info "parachain $2 fullnode $1 is running"
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
	(sh -c "$parachain \
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
	info "parachain $2 collator $1 is running"
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
	info "ready for testing, parachain $1 blocks is produced"
}

function show_message() {
	verbose -c "ls -la $log | grep '\.log$'"
	echo "To view log run command: tail $log/parachain_200_fullnode_0.log"
	if [ $remove_log_dir_on_finalize == 1 ]; then
		echo "hit Ctrl-C to terminate testnet and remove log dir"
	else
		echo "hit Ctrl-C to terminate testnet and keep log dir"
	fi
	info "rococo local test net is running"
}

function run_tests() {
	#polkadot-js-api --seed "//Alice" tx.balances.transfer 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty 999
	return 0
}

function finalize() {
	tailpid=$!
	for pid in $pids
	do
		kill -KILL $pid > /dev/null 2>&1
	done
	tail $log/parachain_200_fullnode_0.log
	if [ $remove_log_dir_on_finalize == 1 ]; then
		rm -Rf $log
	fi
	exit
}

trap finalize 0 1 2 3 6 15

check_dirs_and_files
create_log_dir

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

	$parachain export-genesis-wasm > $log/parachain_${parachain_id}.wasm
	cat $log/parachain_${parachain_id}_collator_0.log | \
		awk "/Parachain genesis state: /{ print \$9; exit }" > $log/genesis_${parachain_id}.txt

	while true; do
		$api \
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
	info "parachain $parachain_id is registred"

done

for parachain_id in $parachains
do
	waiting_for_ready_state $parachain_id
done
show_message
run_tests

wait

