#!/usr/bin/env bash
. `dirname $0`/partial/helpers.sh || exit 1

function restore() {
	cache=$1
	stack=""
	push $2
	while [ "`trim $stack`" != "" ]; do
		commit=`get`
		if file_is_found_and_exist $cache/${commit}*.tar; then
			pop -q
			continue
		fi
		delta=`first_ls $cache/*-$commit.tar.delta`
		if [ -f $delta ]; then
			basis_commit=`basename $delta | awk -F '[\-\.]' '{ print $1 }' 2> /dev/null`
			basis_file=`first_ls $cache/${basis_commit}*.tar`
			if [ -f $basis_file ]; then
				verbose rdiff patch $basis_file $delta $cache/$commit.restored.tar \
					|| panic "patching tar $basis_commit to $commit is failed"
				pop -q
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
		delta="$cache/$old-$new.tar.delta"
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
		restore $2 $3
		break
		;;
	archive)
		archive $2
		break
		;;
	*)
esac
