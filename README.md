# About

Engage is a process composer with ordering and parallelism based on directed
acyclic graphs.

In other words, given a collection of process definitions which can include
ordering dependencies, Engage can run those processes in the order defined,
while also running them in parallel to the extent permitted by the ordering.
Engage supports both "task" processes, which allow dependents of a task to
spawn after the task exits, and "service" processes, which allow dependents of a
service to spawn after the service spawns but before it exits, and normally has
the dependents exit before the service does.

Some related functionality is also provided, such as the ability to run a
subset of the processes rather than all of them and the ability to generate a
visualization of processes and the dependencies between them.

Notably, Engage does not make use of any process isolation features and it runs
all processes as the current user. This deliberate design decision decreases
moving parts between intented behavior and the collective actual behavior of
Engage and the processes it runs. As a result, debugging is simpler and the odds
of an intermediate part being misconfigured or broken are lower.

## External links

* [Chat room](https://zulip.computer.surgery/#narrow/channel/5-engage)
* [Project planning](https://gitlab.computer.surgery/charles/engage/-/issues)
* [Source code contributions](https://gitlab.computer.surgery/charles/engage/-/merge_requests)
* [Source code](https://gitlab.computer.surgery/charles/engage)
* [Website](https://engage.computer.surgery)
