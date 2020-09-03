test "$BASH_VERSION" == "" && exit 1

function info() {
	echo "SCRIPT INFO: $1"
}

function panic() {
	echo "SCRIPT PANIC: $1"
	exit 1
}

function must() {
	$@
	if [ $? != 0 ]; then
		echo "SCRIPT MUST PANIC: line = $BASH_LINENO, command = $@"
		exit 1
	fi
}

function bomb() {
	echo "SCRIPT PANIC BOMB: line = $BASH_LINENO"
	eval "`awk "BEGIN { print \\\"cat <<BoMbbOmB\\\" }
			  { if ($BASH_LINENO - NR <= $1 && NR - $BASH_LINENO <= $2) { print \\$0 } }
		    END   { print \\\"BoMbbOmB\\\" }" < $0`"
	exit 1
}

function command_exist() {
	which $1 > /dev/null 2>&1
}

function on_success() {
	if [ $? == 0 ]; then
		$@
	else
		false
	fi
}

