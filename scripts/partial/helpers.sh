test "$BASH_VERSION" = "" && exit 1

function info() {
	echo "SCRIPT INFO: $1"
}

function panic() {
	echo "SCRIPT PANIC: $1"
	exit 1
}

function must() {
	"$@"
	if [ $? != 0 ]; then
		echo "SCRIPT PANIC: file = `realpath $0` line = $BASH_LINENO"
		echo "> $@"
		exit 1
	fi
}

function bomb() {
	a=$0
	b=$1
	c=$2
	shift; shift
	echo "SCRIPT PANIC: file = `realpath $0` line = $BASH_LINENO"
	eval "`awk "BEGIN { print \\\"cat <<BoMbbOmB\\\" }
			  { if ($BASH_LINENO - NR <= $b && NR - $BASH_LINENO <= $c) { print \\\"> \\\" \\$0 } }
		    END   { print \\\"\\\"; print \\\"BoMbbOmB\\\" }" < $a`"
	exit 1
}

function command_exist() {
	which $1 > /dev/null 2>&1
}

function on_success() {
	if [ $? = 0 ]; then
		"$@"
	else
		false
	fi
}

function firster() {
	awk '{ print $1 }'
}

function first() {
	echo "$@" | firster
}

function first_ls() {
	ls "$@" 2> /dev/null | firster
}

function expect() {
	head -n 1 | grep -qE "$1"
}

function pushd() {
	command pushd "$@" > /dev/null
}

function length() {
	echo "$@" | fmt -w 1 | wc -l
}

function popd() {
	command popd > /dev/null
}

function _get_all_commits() {
	git log --reflog --first-parent | awk '/^commit /{ print $2 }'
}

function get_all_commits() {
	if [ "$top" != "" ]; then
		pushd $top
			_get_all_commits
		popd
	else
		_get_all_commits
	fi
}

function get_current_commit() {
	get_all_commits | head -n 1
}

function unimplemented() {
	panic "not implemented"
}

function trimmer() {
	sed 's,^ *,,g;s,  *, ,g'
}

function trim() {
	echo "$@" | trimmer
}

function get() {
	first $stack
}

function pop() {
	case "$1" in
		-q)
			get
			break
			;;
		-a)
			eval "$2=`get`"
			;;
	esac
	stack=`echo $stack | awk '{ $1=""; print $0 }' | trimmer`
}

function push() {
	stack=`trim $1 $stack`
}

function verbose() {
	echo "SCRIPT RUNNING: $@" | trimmer
	"$@"
}
