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

function opt() {
	if [ "$1" == "" ]; then
		echo $2
	else
		echo $1
	fi
}

function bomb() {
	a=$0
	b=`opt $1 0`
	c=`opt $2 0`
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
	in_shell=0
	if [ "$1" == "-c" ]; then
		shift
		in_shell=1
	fi
	echo "SCRIPT RUNNING: $@" | trimmer
	if [ $in_shell == 1 ]; then
		sh -c "$@"
	else
		"$@"
	fi
}

function file_is_found_and_exist() {
	found=`first_ls "$@"`
	test "$found" == "" && return 1
	test -f $found && echo $found
}

function pwd_is_repos_topdir() {
	test -d .inside_docker_jobs
}

function skip_if_done() {
	mkdir -p ${DONE_DIR}
	pushd ${DONE_DIR}
		test "`bash -c \"ls $@ 2> /dev/null\"`" != "" && exit 0
	popd
}

function set_done()
{
	mkdir -p ${DONE_DIR}
	pushd ${DONE_DIR}
		must touch -f $1
	popd
}

function export_if_not_exist()
{
	if eval "test \"\$$1\" == \"\""; then
		eval "export ${1}=\"${2}\""
	fi
}
