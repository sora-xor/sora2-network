#!/bin/sh
. `dirname $0`/partial/helpers.sh || exit 1

function restore() {
	stack=""
	push $1
	while [ `trim $stack` != "" ]; do
		commit=`get`
		test -f $cache/${commit}*.tar
		if [ $? == 0 ]; then
			pop -q
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
					pop -q
				fi
			else
				push $basis_commit
			fi
		fi
	done
}

function archive() {
	cache=$1
	last=`ls -t $cache | grep exist | head -n 1`
	stack=""
	for commit in `cat $cache/$last`
	do
		tarfile=`first_ls $cache/$commit.point.tar $cache/$commit.fresh.tar`
		test "$tarfile" == "" && continue
		test -f $tarfile || continue
		test -f $cache/$commit.tar.sig || bomb 2 0 "$@"
		push $commit
	done
	ready=0
	while [ $ready != 1 ]; do
		test `length $stack` -ge 2 || return 0
		pop -a new
		pop -a old
		push $old
		delta="$cache/$old-$new.delta"
		if [ ! -f $delta ]; then
			verbose rdiff delta $cache/$old.tar.sig \
			                   `first_ls $cache/${new}*.tar` \
					    $delta.tmp \
				|| bomb 8 0 "$@"
			must mv $delta $delta.tmp
			must rm -f $cache/${new}*.tar
		fi
	done
}

case "$1" in
	restore)
		restore $2
		break
		;;
	archive)
		archive $2
		break
		;;
	*)
esac
