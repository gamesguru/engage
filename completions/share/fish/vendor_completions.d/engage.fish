# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_engage_global_optspecs
	string join \n f/file= l/log-format= p/process= h/help V/version
end

function __fish_engage_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_engage_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_engage_using_subcommand
	set -l cmd (__fish_engage_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c engage -n "__fish_engage_needs_command" -s f -l file -d 'Manually select the Engage file' -r -F
complete -c engage -n "__fish_engage_needs_command" -s l -l log-format -d 'Select the log format' -r -f -a "default\t'The default format'
compact\t'[`tracing_subscriber`]\'s compact format'
full\t'[`tracing_subscriber`]\'s full format'
pretty\t'[`tracing_subscriber`]\'s pretty format'
json\t'[`tracing_subscriber`]\'s JSON format'"
complete -c engage -n "__fish_engage_needs_command" -s p -l process -d 'Select a process' -f -a "(engage list --relaxed 2>/dev/null)"
complete -c engage -n "__fish_engage_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c engage -n "__fish_engage_needs_command" -s V -l version -d 'Print version'
complete -c engage -n "__fish_engage_needs_command" -f -a "completions" -d 'Print completions for a supported shell'
complete -c engage -n "__fish_engage_needs_command" -f -a "dot" -d 'Print a graph in Graphviz\' DOT language of processes and their dependencies'
complete -c engage -n "__fish_engage_needs_command" -f -a "list" -d 'Print available processes'
complete -c engage -n "__fish_engage_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c engage -n "__fish_engage_using_subcommand completions" -s h -l help -d 'Print help'
complete -c engage -n "__fish_engage_using_subcommand dot" -s f -l file -d 'Manually select the Engage file' -r -F
complete -c engage -n "__fish_engage_using_subcommand dot" -s p -l process -d 'Select a process' -f -a "(engage list --relaxed 2>/dev/null)"
complete -c engage -n "__fish_engage_using_subcommand dot" -s r -l relaxed -d 'Treat certain kinds of errors as warnings'
complete -c engage -n "__fish_engage_using_subcommand dot" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c engage -n "__fish_engage_using_subcommand list" -s f -l file -d 'Manually select the Engage file' -r -F
complete -c engage -n "__fish_engage_using_subcommand list" -s r -l relaxed -d 'Treat certain kinds of errors as warnings'
complete -c engage -n "__fish_engage_using_subcommand list" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c engage -n "__fish_engage_using_subcommand help; and not __fish_seen_subcommand_from completions dot list help" -f -a "completions" -d 'Print completions for a supported shell'
complete -c engage -n "__fish_engage_using_subcommand help; and not __fish_seen_subcommand_from completions dot list help" -f -a "dot" -d 'Print a graph in Graphviz\' DOT language of processes and their dependencies'
complete -c engage -n "__fish_engage_using_subcommand help; and not __fish_seen_subcommand_from completions dot list help" -f -a "list" -d 'Print available processes'
complete -c engage -n "__fish_engage_using_subcommand help; and not __fish_seen_subcommand_from completions dot list help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
