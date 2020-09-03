#!/bin/sh

function trim() {
	echo $1 | sed 's,^ *,,g;s,  *, ,g'
}

function readc() {
	echo $stack | awk '{ print $1 }'
}

function popc() {
	stack=`echo $stack | awk '{ $1=""; print $0 }' | trim`
	readc
}

function pushc() {
	stack="echo $1 $stack | trim"
}

function restore() {
	stack=""
	pushc $1
	while [ `trim $stack` != "" ]; do
		commit=`readc`
		test -f $cache/${commit}*.tar
		if [ $? == 0 ]; then
			popc > /dev/null
			continue
		fi
		delta=`echo $cache/$commit-*.tar.delta | fmt -w 1 | head -n 1`
		if [ -f $delta ]; then
			basis_commit=`echo $delta | awk -F '[\-\.]' '{ print $2 }' 2> /dev/null`
			basis_file=`echo ${basis_commit}*.tar | fmt -w 1 | head -n 1`
			if [ -f $basis_file ]; then
				rdiff patch $basis_file $delta $cache/$commit.restored.tar
				if [ $? != 0 ]; then
					echo "SCRIPT: patching tar $basis_commit to $commit is failed"
					exit 1
				else
					popc > /dev/null
				fi
			else
				pushc $basis_commit
			fi
		fi
	done
}

if [ "$1" == "restore" ]; then
	restore $2
fi
