# About

Engage is a process composer with ordering and parallelism based on directed
acyclic graphs.

Given a file that configures a set of processes, including but not limited
to dependencies between each process, the `engage` command line program can
run all or a subset of the processes ordered by their dependencies, list
available processes, or be used to generate a visualization of the dependency
graph. Engage does not make use of any process isolation features and runs all
processes as the current user, as its purpose is simply to automate running a
set of commands in a specific order and in parallel where possible. This design
puts relatively few moving parts between your intent and the actual behavior,
which makes debugging simpler and reduces the odds of an intermediate part being
misconfigured or broken.

## External links

* [Chat room](https://zulip.computer.surgery/#narrow/channel/5-engage)
* [Project planning](https://gitlab.computer.surgery/charles/engage/-/issues)
* [Source code contributions](https://gitlab.computer.surgery/charles/engage/-/merge_requests)
* [Source code](https://gitlab.computer.surgery/charles/engage)
* [Website](https://engage.computer.surgery)
