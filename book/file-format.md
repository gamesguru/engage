# File format

Engage files are in the [TOML] format. The structure that Engage uses is
described below.

[TOML]: https://toml.io

## `interpreter`

* Required: yes.
* Type: list of strings.

This top-level key is used to set the interpreter used to execute the `script`
of each task. A typical value is `["bash", "-euo", "pipefail", "-c"]`.

## `[tasks.<name>]`

This table defines a task. The name of the task must be provided in place of
`<name>`. All task names within an Engage file must be unique.

### `script`

* Type: string.
* Required: yes.

This value gets appended to the list defined by `interpreter` and then executed
after this task's closure of dependencies has been fulfilled.

### `env`

* Type: map of strings to strings.
* Required: no.

Extra environment variables to set for the script process. Values provided here
will take precedence over any ambient environment variable of the same name.

For example, `env.FOO = "foo"` will set the environment variable named `FOO` to
the value `foo`.

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
