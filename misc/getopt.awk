BEGIN {
	start=0
	long=""
	short=""
	longsep=""
	usage_txt=""
	shell=""
	newline=""
	codesep=""
}

{
	usage=1
	if (substr($1,1,1) == "-") {
		start=1
	} else {
		if (start) {
			usage=0
		}
	}
	if (usage) {
		usage_txt = usage_txt newline $0
		newline="\n"
	}
	if (start) {
		if (usage) {
			with_arg=0
			if (match($2, /=/)) {
				with_arg=1
				a = gensub(/--(.+)=(.+)/, "\\1", "g", $2)
				b = substr($1,2,1)
				long  = long longsep a ":"
				short = short b ":"
			} else {
				a = gensub(/--(.+)/, "\\1", "g", $2)
				b = substr($1,2,1)
				long  = long longsep a
				short = short b
			}
			shell = shell codesep
			shell = shell "-" b "|--" a ")\n"
			v = gensub(/-/, "_", "g", a)
			if (with_arg) {
				shell = shell "     shift\n"
				shell = shell "     " v "=$1\n"
			} else {
				shell = shell "     " v "=1\n"
			}
			longsep = ","
			codesep = "     ;;\n"
		} else {
			shell = shell "     " $0 "\n"
		}
	}
}

END {
	shell = shell codesep
	print "usage() {"
	print "cat <<EOF"
	print usage_txt
	print "EOF"
	print "}"
	print "options=$(getopt -l \"" long "\" -a -o \"" short "\" -- \"$@\") || exit 1"
	print "eval set -- \"$options\""
	print "while true; do"
	print "  case $1 in"
	print shell
	print "--)"
    	print "    shift"
        print "    break;;"
	print "  esac"
	print "  shift"
	print "done"
}
