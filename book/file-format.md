# File format

Engage files are in the [TOML] format. The structure that Engage uses is
described below.

[TOML]: https://toml.io

## `version`

* Type: string.
* Required: yes.

A SemVer version requirement which constrains the set of compatible versions
of the Engage file format's syntax and semantics (not the version of Engage
itself).

The syntax of this value is exactly that of [`<valid semver>` from the SemVer
grammar][semver-grammar].

The lower bound on compatible versions is equal to the provided value, and
the upper bound is set by requiring the first nonzero `<numeric identifier>`
in `<version core>` be equal while allowing any subsequent `<numeric
identifier>`s in `<version core>` to increase. If the version requirement
contains `<pre-release>`, the sole compatible version is equal to the version
requirement. `<build>` is always ignored. These rules can be roughly understood
as "any compatible version equal to or newer than the provided value fulfills
the requirement".

If the version of Engage in use does not support any versions of the file format
that fulfill the requirement, Engage will refuse to load the file. This ensures
that Engage files are interpreted as intended as Engage evolves, and makes
it possible to produce useful error messages when Engage files require syntax
and/or semantics that the version of Engage in use does not implement.

[semver-grammar]: https://semver.org/#backusnaur-form-grammar-for-valid-semver-versions

## `[tasks.<name>]`

This table defines a task. The name of the task must be provided in place of
`<name>`. All task names within an Engage file must be unique. The name must
match `[a-z0-9-]+` and `-` cannot be the first character.

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
