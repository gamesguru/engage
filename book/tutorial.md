# Tutorial

This section demonstrates common features of Engage. After understanding this
section, the reference-style documentation (e.g. command line help text and
[Engage file format](file-format.md)) should be sufficient for learning the
other available features.

## Choosing the Engage file

By default, Engage will search for a file called `engage.toml` in the current
directory or all of its parent directories. Alternatively, a file can be
specified via a command line option. Henceforth, the phrase "Engage file" means
the file chosen by either of those strategies.

## Adding tasks

An Engage file is pointless without any tasks, so let's add a few:

```toml
{{#include assets/tutorial.toml:1:5}}
```

Each `[tasks.<name>]` table defines a task, where `<name>` is a placeholder for
the actual name of the task. `command` is the only required field for each task.

Task names are useful for identifying which part of the Engage file is producing
what output, visualizing the dependency graph, and selecting a subset of the
tasks to run at the command line. It's idiomatic for task names to be nouns that
don't contain characters that a shell would treat specially.

When Engage runs these tasks, the empty files `foo` and `bar` will be created
(or their atime and mtime will be updated if they already exist) in the same
directory as the Engage file, not in the working directory the `engage` command
was executed in (although these may be the same directory).

## Adding tasks with dependencies

Let's say we want to clean up after ourselves by deleting these files before
exiting:

```toml
{{#include assets/tutorial.toml:7:13}}
```

Note the use of `after` for both of these tasks; this is required to ensure
that they run after, rather than the default of running in parallel with, the
`create-foo` and `create-bar` tasks.

Now let's say we want to do something between creating and deleting these files;
for example, we'll just call `stat` on both of them:

```toml
{{#include assets/tutorial.toml:15:18}}
```

Note the use of `after` and `before`; this is what achieves the desired
"between" semantics. The advantage of having and using both `after` and `before`
rather than only one or the other is that this allows better organization of
ordering constraints. For example, if we wanted to add or remove tasks that run
between creation and deletion and only had or used `after`, the deletion tasks
would have to be updated for each of those changes, whereas this way, only the
tasks being added or removed need to be modified.

## Putting it all together

Incorporating all the changes from the previous three sections results in this
complete Engage file:

```toml
{{#include assets/tutorial.toml}}
```

## Visualizing task dependencies

The `engage dot` subcommand can be used to convert an Engage file into [Graphviz
DOT Language][dot], which can then be processed by other tools. For example, it
can be used to produce a graph like this from the above Engage file:

![Graph of the tutorial Engage file](assets/tutorial.svg)

This illustrates the order in which Engage will run each task, what tasks can
be run in parallel with each other, and which of `before` and `after` created
each dependency edge. This can be useful for debugging dependency cycles or
unexpected ordering between tasks in general.

In this graph, `create-foo` and `create-bar` will run in parallel, then
`stat-both` will run by itself, and finally `delete-foo` and `delete-bar`
will run in parallel. You may also notice that there are "redundant" ordering
requirements (edges) in this graph, for example, `create-foo -> delete-foo` is
redundant with `create-foo -> stat-both -> delete-foo`. While the presence of
`create-foo -> delete-foo` has no effect on ordering, it is useful to retain it
so that if `stat-both` (and thus its ordering requirements) are removed in the
future, the intent of running `delete-foo` after `create-foo` is preserved.

[dot]: https://graphviz.org/doc/info/lang.html

## Running tasks

The `engage` command, when run with no subcommand, will execute all tasks in the
selected Engage file. While doing so, Engage will forward `stdout` and `stderr`
of the tasks, alongside an indication of which task is generating the output,
whether the output is coming from the task's `stdout` or `stderr` (marked with
an `O` or `E`, respectively), and some extra fluff to make it look pretty. At
the end, Engage will print out whether the run succeeded or failed and exit with
an appropriate status code:

| Status code | Meaning |
|-|-|
| `0` | All tasks exited successfully. |
| `1` | At least one task exited with an error status code. |
| `2` | Other errors, such as issues with the Engage file. |

Note that Engage's own `stdout` and `stderr` output is not considered stable.

It's also possible to use the `engage just` subcommand to run a subset of the
tasks in an Engage file.

If Engage receives the `SIGINT` signal (e.g. via `ctrl`+`c`) while running
tasks, it will prevent any more tasks from starting, send `SIGINT` to any
running tasks, and wait for them to finish before exiting.
