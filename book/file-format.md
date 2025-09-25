# File format

Engage files are in the [TOML] format. The structure that Engage uses is
described below.

[TOML]: https://toml.io

## `[tasks.<name>]`

This table defines a task. The name of the task must be provided in place of
`<name>`. All task names within an Engage file must be unique.

### `command`

* Type: list of strings.
* Required: yes.

The command to run when all of this task's dependencies have completed.

### `environment`

* Type: map of strings to strings.
* Required: no.

Extra environment variables to set when running `command`. Values provided here
will take precedence over any ambient environment variable of the same name.

For example, `environment.FOO = "foo"` will set the environment variable named
`FOO` to the value `foo`.

### `after`

* Type: list of strings.
* Required: no.

Can be set to a list of task names that must complete successfully before this
task can be started.

### `before`

* Type: list of strings.
* Required: no.

Can be set to a list of task names that will only be started after this task has
completed successfully.
