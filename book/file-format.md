# File format

Engage files are in the [TOML] v1.1.0 format. The keys and values thereof that
Engage uses and their effects are described below.

[TOML]: https://toml.io

## `processes` key

* Type: Table whose values are [process tables](#process-table).
* Required: No.

This table defines the set of processes. The keys in this table define the name
of each process, which must match `^[a-z0-9-]+$` and cannot start with `-`. Each
key's value defines the configuration for that process.

### Process table

#### `command` key

* Type: Array of strings.
* Required: Yes.

The command used to spawn this process after all of its dependencies have exited
successfully.

#### `environment` key

* Type: Table whose values are strings.
* Required: No.

Environment variables to add or override for this process. Each key-value
pair in this table defines the name of an environment variable and its value
respectively. Environment variables not defined in this table are left unset or
are inherited normally.

#### `after` key

* Type: Array of strings.
* Required: No.

Names of processes that must exit successfully before this process can be
spawned. Each name may appear more than once, though this has no additional
effect.

#### `before` key

* Type: Array of strings.
* Required: No.

Names of processes that must only be spawned after this process has exited
successfully. Each name may appear more than once, though this has no additional
effect.
