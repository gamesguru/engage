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
supposed to choose) is forbidden and will cause Engage exit with an error.

[TOML]: https://toml.io

<!-- NOTE: Keep sections sorted lexicographically by path. -->

## `.processes`

Type
: Table of tables.

Applicability
: Optional.

Description
: Defines the set of processes. The keys in this table define the name of each
  process, which must match the regex `^[a-z0-9][a-z0-9-]*$`. Each key's value
  defines the configuration for that process.

Examples
: Empty file
  : ```toml
    ```

    This Engage file defines no processes.

  Table without keys
  : ```toml
    [processes]
    ```

    This Engage file defines no processes.

  One process
  : ```toml
    [processes.hello-world]
    command = ["echo", "Hello, world!"]
    ready-when = "exited"
    ```

    This Engage file defines one process. The process:

    * Is named `hello-world`.
    * Defines a command that prints `Hello, world!` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.

    When running all processes in this Engage file, the following will happen:

    1. The `hello-world` process spawns, because it has no dependencies.
    2. The `hello-world` process prints `Hello, world!` to its stdout.
    3. The `hello-world` process exits successfully, thus becoming ready.

## `.processes.*.after`

Type
: Array of strings.

Applicability
: Optional.

Description
: Defines that this process must be spawned after processes named in this array
  become ready. Each process name may appear more than once, though this has no
  additional effect.

Examples
: One dependency
  : ```toml
    [processes.first]
    command = ["echo", "Hello"]
    ready-when = "exited"

    [processes.second]
    command = ["echo", "Goodbye"]
    ready-when = "exited"
    after = ["first"]
    ```

    This Engage file defines two processes. One process:

    * Is named `first`.
    * Defines a command that prints `Hello` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.

    The other process:

    * Is named `second`.
    * Defines a command that prints `Goodbye` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.
    * Spawns after `first` becomes ready.

    When running all processes in this Engage file, the following will happen:

    1. The `first` process spawns, because it has no dependencies.
    2. The `first` process prints `Hello` to its stdout.
    3. The `first` process exits successfully, thus becoming ready.
    4. The `second` process spawns, because its dependencies are ready.
    5. The `second` process prints `Goodbye` to its stdout.
    6. The `second` process exits successfully, thus becoming ready.

## `.processes.*.before`

Type
: Array of strings.

Applicability
: Optional.

Description
: Defines that this process must become ready before processes named in this
  array can be spawned. Each process name may appear more than once, though this
  has no additional effect.

Examples
: One dependency
  : ```toml
    [processes.first]
    command = ["echo", "Hello"]
    ready-when = "exited"
    before = ["second"]

    [processes.second]
    command = ["echo", "Goodbye"]
    ready-when = "exited"
    ```

    This Engage file defines two processes. One process:

    * Is named `first`.
    * Defines a command that prints `Hello` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.
    * Becomes ready before `second` is spawned.

    The other process:

    * Is named `second`.
    * Defines a command that prints `Goodbye` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.

    When running all processes in this Engage file, the following will happen:

    1. The `first` process spawns, because it has no dependencies.
    2. The `first` process prints `Hello` to its stdout.
    3. The `first` process exits successfully, thus becoming ready.
    4. The `second` process spawns, because its dependencies are ready.
    5. The `second` process prints `Goodbye` to its stdout.
    6. The `second` process exits successfully, thus becoming ready.

## `.processes.*.command`

Type
: Array of strings.

Applicability
: Required.

Description
: Defines the command used to spawn this process after its dependencies become
  ready. The array must have at least one value. The first value determines
  both the program to spawn a process for as well as the first argument to that
  process.

Examples
: Program with no additional arguments
  : ```toml
    [processes.minimal]
    command = ["true"]
    ready-when = "exited"
    ```

    This Engage file defines one process. The process:

    * Is named `minimal`.
    * Defines a command that exits successfully.
    * Becomes ready when it exits successfully.

    When running all processes in this Engage file, the following will happen:

    1. The `minimal` process spawns, because it has no dependencies.
    2. The `minimal` process exits successfully, thus becoming ready.

  Program with additional arguments
  : ```toml
    [processes.hello-world]
    command = ["echo", "Hello, world!"]
    ready-when = "exited"
    ```

    This Engage file defines one process. The process:

    * Is named `hello-world`.
    * Defines a command that prints `Hello, world!` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.

    When running all processes in this Engage file, the following will happen:

    1. The `hello-world` process spawns, because it has no dependencies.
    2. The `hello-world` process prints `Hello, world!` to its stdout.
    3. The `hello-world` process exits successfully, thus becoming ready.

## `.processes.*.environment`

Type
: Table of strings.

Applicability
: Optional.

Description
: Defines environment variables to set for this process. Each key-value pair
  in this table defines the name of an environment variable and its value
  respectively. Environment variables not defined in this table are left unset
  or are inherited normally.

Examples
: Set one environment variable
  : ```toml
    [processes.hello-world]
    environment.NAME = "world"
    command = ["bash", "-c", "echo Hello, $NAME\!"]
    ready-when = "exited"
    ```

    This Engage file defines one process. The process:

    * Is named `hello-world`.
    * Sets an environment variable named `NAME` to `world`.
    * Defines a command that prints `Hello, world!` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.

    When running all processes in this Engage file, the following will happen:

    1. The `hello-world` process spawns, because it has no dependencies.
    2. The `hello-world` process prints `Hello, world!` to its stdout.
    3. The `hello-world` process exits successfully, thus becoming ready.

## `.processes.*.ready-when`

Type
: One of `"exited"` and `"spawned"`.

Applicability
: Required.

Description
: Defines the point at which this process is considered ready. In turn, this
  defines when processes that depend on this one can be spawned, as well as the
  order in which this process and its dependents exit.

  A value of `"exited"` causes dependent processes to be started after this
  process has exited successfully.

  A value of `"spawned"` causes dependent processes to be started after this
  process has successfully spawned. It also causes this process to exit after
  its dependents have exited.

Examples
: Two tasks
  : ```toml
    [processes.first]
    command = ["echo", "Hello"]
    ready-when = "exited"

    [processes.second]
    command = ["echo", "Goodbye"]
    ready-when = "exited"
    after = ["first"]
    ```

    This Engage file defines two processes. One process:

    * Is named `first`.
    * Defines a command that prints `Hello` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.

    The other process:

    * Is named `second`.
    * Defines a command that prints `Goodbye` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.
    * Spawns after `first` becomes ready.

    When running all processes in this Engage file, the following will happen:

    1. The `first` process spawns, because it has no dependencies.
    2. The `first` process prints `Hello` to its stdout.
    3. The `first` process exits successfully, thus becoming ready.
    4. The `second` process spawns, because its dependencies are ready.
    5. The `second` process prints `Goodbye` to its stdout.
    6. The `second` process exits successfully, thus becoming ready.

  One task and one service
  : ```toml
    [processes.service]
    command = ["sleep", "infinity"]
    ready-when = "spawned"

    [processes.task]
    command = ["echo", "Hello, world!"]
    ready-when = "exited"
    after = ["service"]
    ```

    This Engage file defines two processes. One process:

    * Is named `service`.
    * Defines a command that sleeps forever.
    * Becomes ready when it spawns.

    The other process:

    * Is named `task`.
    * Defines a command that prints `Hello, world!` to its stdout and exits
      successfully.
    * Becomes ready when it exits successfully.
    * Spawns after `service` becomes ready.

    When running all processes in this Engage file, the following will happen:

    1. The `service` process spawns, because it has no dependencies.
    2. The `service` process sleeps forever, in parallel with steps 3 through 5.
    3. The `task` process spawns, because its dependencies are ready.
    4. The `task` process prints `Hello, world!` to its stdout.
    5. The `task` process exits successfully, thus becoming ready.
    6. Engage begins ending the run by sending `SIGINT` to the `service` process
       because all processes without dependents are tasks that have exited
       successfully.
    6. The `service` process exits because of the `SIGINT` it received from
       Engage.
    7. Engage exits with an error because `service` exited due to an unhandled
       signal.
