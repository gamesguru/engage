* All task scripts are executed with the working directory set to the location of the Engage file.

* Subcommands that require the Engage file can be executed from any directory so long as either the current directory or any of its ancestors contain the Engage file.

* Group and task dependencies must form a directed acyclic graph; Engage will enforce this. In other words, dependency cycles are not allowed.

* If a task fails, any subsequent tasks will not be executed and Engage will exit with the same value as the failed task.

* If no subcommand is supplied, all groups and tasks will be scheduled based on their dependencies and executed appropriately.
