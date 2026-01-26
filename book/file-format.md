# File format

Engage files are in the [TOML] v1.1.0 format.

The following sections are named after the "path" to the key they document.
Paths are formed by an initial `.` followed by zero or more key names separated
by `.`, where subsequent keys belong to the table named by the previous key.
The key name `*` is a placeholder which indicates that there may be zero or more
keys in its place and that you are supposed to choose the names of those keys.
Key names suffixed with `[]` indicate that the key's value is an array of tables
and the key name after the following `.`, if any, belongs to each table within
the array.

The use of key names not documented here (aside from keys whose names you are
supposed to choose) is forbidden and will cause Engage emit an error and exit
with code 2.

[TOML]: https://toml.io

<!-- NOTE: Keep sections sorted lexicographically by path. -->

## `.processes`

Type
: Table of tables.

Applicability
: Optional.

Description
: This table defines the set of processes. The keys in this table define the
  name of each process, which must match `^[a-z0-9-]+$` and cannot start with `-`.
  Each key's value defines the configuration for that process.

Examples
: Empty file
  : ```toml
    ```
    This file defines no processes.

  Table without keys
  : ```toml
    [processes]
    ```
    This file defines no processes.

  One process
  : ```toml
    [processes.hello-world]
    command = ["echo", "Hello, world!"]
    ```
    This file defines one process named `hello-world` that prints `Hello,
    world!` to stdout and exits successfully.

## `.processes.*.after`

Type
: Array of strings.

Applicability
: Optional.

Description
: Names of processes that must exit successfully before this process can be
  spawned. Each process name may appear more than once, though this has no
  additional effect.

Examples
: One dependency
  : ```toml
    [processes.first]
    command = ["echo", "Hello"]

    [processes.second]
    command = ["echo", "Goodbye"]
    after = ["first"]
    ```
    This file defines the two processes `first` and `second`, where `first`
    is spawned first and `second` is only spawned after `first` exits
    successfully. `Hello` will be printed to stdout by `first` which will then
    exit successfully, followed by `Goodbye` being printed to stdout by `second`
    which will then exit successfully.

## `.processes.*.before`

Type
: Array of strings.

Applicability
: Optional.

Description
: Names of processes that must only be spawned after this process has exited
  successfully. Each process name may appear more than once, though this has no
  additional effect.

Examples
: One dependency
  : ```toml
    [processes.first]
    command = ["echo", "Hello"]
    before = ["second"]

    [processes.second]
    command = ["echo", "Goodbye"]
    ```
    This file defines the two processes `first` and `second`, where `first`
    is spawned first and `second` is only spawned after `first` exits
    successfully. `Hello` will be printed to stdout by `first` which will then
    exit successfully, followed by `Goodbye` being printed to stdout by `second`
    which will then exit successfully.

## `.processes.*.command`

Type
: Array of strings.

Applicability
: Required.

Description
: The command used to spawn this process after all of its dependencies have
  exited successfully. The array must have at least one value. The first value
  determines both the program to spawn a process for as well as the first
  argument to that process.

Examples
: Program with no additional arguments
  : ```toml
    [processes.minimal]
    command = ["true"]
    ```
    This file defines one process named `minimal` that prints nothing and exits
    successfully.

  Program with additional arguments
  : ```toml
    [processes.hello-world]
    command = ["echo", "Hello, world!"]
    ```
    This file defines one process named `hello-world` that prints `Hello,
    world!` to stdout and exits successfully.

## `.processes.*.environment`

Type
: Table of strings.

Applicability
: Optional.

Description
: Environment variables to set for this process. Each key-value pair in this
  table defines the name of an environment variable and its value respectively.
  Environment variables not defined in this table are left unset or are
  inherited normally.

Examples
: Set one environment variable
  : ```toml
    [processes.hello-world]
    command = ["bash", "-c", "echo Hello, $NAME\!"]
    environment.NAME = "world"
    ```
    This file defines one process named `hello-world` that prints `Hello,
    world!` to stdout by reading part of the output from the configured
    environment variable and exits successfully.
