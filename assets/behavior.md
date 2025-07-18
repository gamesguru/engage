* All task scripts are executed with the working directory set to the location of the Engage file.

* Operations that require the Engage file can be invoked from the directory it's in or any of that directory's children.

* Group and task dependencies must form a directed acyclic graph. In other words, dependency cycles are not allowed.

* If a task fails, any dependent tasks will not be executed and Engage will exit with a status of `1`.

* If some other error occurs (e.g. configuration error), Engage will exit with a status of `2`.

* If no subcommand is supplied, all groups and tasks will be scheduled and executed based on their dependencies.

<!-- markdownlint-disable-file MD013 MD041 -->
