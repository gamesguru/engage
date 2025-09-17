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

### `depends`

* Type: list of strings.
* Required: no.

Can be set to a list of task names that must complete successfully before this
task can be started.

### `ignore`

* Type: list of integers.
* Required: no.

Can be used to set a list of exit codes to treat as successful. `0` is always
considered successful.
