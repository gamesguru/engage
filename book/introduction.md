# Introduction

This page introduces the most important concepts in Engage, but does not cover
everything; for the remainder, see the rest of the pages in the "For users"
category and the command line help text.

## Selecting the Engage file

Engage will search for a file named `engage.toml` in the current directory and
all of its parent directories, and it will select the one closest to the current
directory, or exit with an error if it can't find one. Alternatively, the path
to a file can be specified on the command line. The term "Engage file" refers to
the file selected in either of these ways.

## Process basics

Processes are Engage's unit of work. An Engage file can contain zero or more
process definitions, even though zero is an unhelpful number of processes.
Each process defines a command that can be spawned into an OS process which in
turn does the aforementioned work. When the OS process is spawned, its working
directory will be set to the parent directory of the Engage file the process
is defined in, rather than inheriting the working directory from Engage's own
OS process.

## Process dependencies

Processes can depend on one or more other processes. The primary reason to
define dependencies between processes is to control the order in which those
processes spawn and exit. A process `x` can be made to depend on another process
`y` either by configuring `x` to be ordered after `y`, by configuring `y` to
be ordered before `x`, or both. Ordering, and thus dependencies, are configured
per-process with the [`.processes.*.before`] and [`.processes.*.after`] keys.

It is safe, and often desirable, to have direct or indirect dependencies that
appear to be "redundant". A redundant dependency is one whose addition or
removal would have no effect on the order in which processes spawn and exit. A
major reason redundant dependencies are desirable is to reduce the likelihood
of accidental ordering changes as processes are added, removed, and edited over
time.

However, Engage will exit with an error before spawning any processes if it
would have to spawn one or more processes that, directly or indirectly, depend
on themselves (this is often called a "dependency cycle").

Engage will run processes in parallel to the extent permitted by their
dependencies. For example:

* Three processes with no dependencies will be run in parallel with each other.
* Given the processes `x`, `y`, and `z` where `z` depends on `y`, `y` will run
  before `z`, and `x` will run in parallel with `y` and `z`.
* Given the processes `x`, `y`, and `z` where `z` depends on `x` and `y`, `x`
  and `y` will run in parallel, and `z` will run after `x` and `y`.

This can be a significant performance improvement over running all processes
in serial.

[`.processes.*.after`]: ./file-format.html#processesafter
[`.processes.*.before`]: ./file-format.html#processesbefore

## Process readiness

Processes have a concept called "readiness" which refers to the point in time
at which a process permits other processes that depend on it to be spawned. A
process "becomes ready" when it reaches that point in time. The point in time is
configured per-process with the [`.processes.*.ready-when`] key.

Based on this concept, processes are split into two kinds: "tasks" and
"services". A task is a process that can become ready by exiting successfully.
A service is a process that can become ready after it spawns but before it
exits. Another major difference between tasks and services is the order in which
interdependent processes of each kind can spawn and exit. The behavior of each
kind is explained in the following two sections.

[`.processes.*.ready-when`]: ./file-format.html#processesready-when

## Tasks

A task is a process that can become ready by exiting successfully.

Processes that depend on tasks will only be spawned after those tasks have
exited successfully.

A task that fails to spawn or exits unsuccessfully will prevent any processes
that depend on it from spawning.

For example, given the tasks `x`, `y`, and `z` where `z` depends on `y` and `y`
depends on `x`, the following will happen when these processes are run:

1. `x` spawns, because it has no dependencies.
2. `x` exits successfully, thus becoming ready.
3. `y` spawns, because its dependencies are ready.
4. `y` exits successfully, thus becoming ready.
5. `z` spawns, because its dependencies are ready.
6. `z` exits successfully, thus becoming ready, but its readiness isn't relevant
   here.

## Services

A service is a process that can become ready after it spawns but before it
exits.

Processes that depend on services will normally exit before those services exit;
consequently, this means that interdependent services will normally exit in the
reverse order from which they spawned. Abnormally, a service may exit before
processes depending on it exit if the service decides to exit early of its own
accord; for example, as a result of a misconfiguration or other runtime error.

A service that fails to spawn will prevent any processes that depend on it from
spawning.

For example, given the services `x` and `y` and the task `z` where `z` depends
on `y` and `y` depends on `x`, the following will happen when these processes
are run:

1. `x` spawns, because it has no dependencies, thus becoming ready.
2. `y` spawns, because its dependencies are ready, thus becoming ready.
3. `z` spawns, because its dependencies are ready.
4. `z` exits successfully, thus becoming ready, but its readiness isn't relevant
   here.
5. `y` exits successfully.
6. `x` exits successfully.

## Beginning a run

The `engage` command (without any subcommand) will attempt to spawn all
processes in the selected Engage file, starting with processes without
dependencies and spawning subsequent processes as their dependencies become
ready. If specified, the `-p`/`--process` option will cause Engage to only
attempt to spawn the selected processes and their dependencies.

## During a run

While running processes, Engage will forward stdout and stderr of those
processes to Engage's stdout, alongside an indication of which process emitted
which output and whether the output came from the process' stdout or stderr. In
the default log format, whether a process' output came from its stdout or stderr
is indicated by an `O` or an `E` in Engage's stdout, respectively.

## Ending a run

If any of the following happen while Engage is running:

* Engage receives `SIGINT`.
* Any process fails to spawn or exits unsuccessfully.
* All processes without dependents are tasks, and they have exited successfully.

Then Engage will send `SIGINT` once to each running process that either has no
dependents or whose dependents have already exited, wait for those processes to
exit, then repeat these steps until all processes have exited. For the avoidance
of doubt, this does preserve the behavior of services normally exiting in the
reverse order from which they spawned.

Notably, if any process without dependents is a service and all processes run
successfully, Engage will continue running until it receives `SIGINT`.

When no more processes are running, Engage will print out whether the run ended
in success or failure and then exit with an appropriate exit status.

Engage will also stop running under various other conditions, including but not
limited to:

* The laptop it's running on depletes its battery.
* The desktop it's running on has a power outage due to inclement weather.
* The gaming desktop it's running on catches on fire due to 12VHPWR.
* The datacenter it's running on catches on fire due to a water leak.
* The planet it's running on gets enveloped by its star(s).
* The inevitable heat death of the universe.

## Exit statuses

The following definition list enumerates the exit statuses emitted by Engage and
their meanings.

0
: The command completed successfully.

1
: The command attempted to run processes and at least one process' exit status
  indicates an error.

2
: The command encountered other errors, such as issues with the Engage file or
  invalid command line arguments.

## API stability

The following sections enumerate which parts of Engage may and may not
experience breaking changes between SemVer-compatible versions.

### Stable

* The command line arguments' syntax, structure, and semantics.
* The file format's syntax, structure, and semantics.
* The semantics of existing exit statuses.

### Unstable

* The format of output written to stdout and stderr.
