# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog][keep_a_changelog], and this project
adheres to [SemVer][semver].

[keep_a_changelog]: https://keepachangelog.com/en/1.0.0/
[semver]: https://semver.org/spec/v2.0.0.html

## Unreleased

### Added

* Added a command to emit a JSON Schema document describing Engage files (<https://or.computer.surgery/charles/engage/-/commit/1a0822d69d955ad2c09bb9d4bd8090ee5a0795f4>)
* Added more thorough documentation in the form of a book (too many commits
  to link)

### Fixed

* Only load the Engage file when necessary (<https://or.computer.surgery/charles/engage/-/commit/2f394adbecd54d66d8c4df8b8b6021f7faf2e09b>)

## v0.2.0 - 2023-09-19

### Changed

* **BREAKING:** Always exit with a status of `1` when a task fails instead of
  trying to exit with the same status code as the task (<https://or.computer.surgery/charles/engage/-/commit/c18357eea2bba8963b0747a667efcff85039dea3>)
* **BREAKING:** Exit with a status of `2` for errors other than tasks failing (<https://or.computer.surgery/charles/engage/-/commit/a991d12777d297f6d4d4c756e7eb349d2ff39720>)
* **BREAKING:** Move subcommands from under `self` to the top level and remove
  the `self` subcommand. For example, you'd now use `engage dot` instead of
  `engage self dot`. (<https://or.computer.surgery/charles/engage/-/commit/e537e9dda930fca1a6864c27c3367bbca399c14e>)
* There are no longer any restrictions on group names (<https://or.computer.surgery/charles/engage/-/commit/0850054aececd133a84c8ad8a0dfb480bf79f035>)

### Fixed

* Report all failed tasks instead of just the first one (<https://or.computer.surgery/charles/engage/-/commit/b9fcdcdf5d0fdcae426839bb36b6a22cd5b3650f>)

## v0.1.3 - 2023-09-10

### Fixed

* Wait for all started tasks to complete before terminating (<https://or.computer.surgery/charles/engage/-/commit/d98c6cc552b979256a85cd79de898e34963fbdc8>)

## v0.1.2 - 2023-02-10

### Added

* Add a `--jobs`/`-j` option to limit parallelism (<https://or.computer.surgery/charles/engage/-/commit/da1cc613e0b535fb7499a441f40011939452fda5>)

### Changed

* Improve behavior summary (<https://or.computer.surgery/charles/engage/-/commit/7be43e4e11ca50dc8415f184865eb0a65d585375>)

## v0.1.1 - 2023-01-28

### Added

* Added a subcommand to generate shell completions (<https://or.computer.surgery/charles/engage/-/commit/48c925bb0c7ab216c72e7ab47891a2a6c26cc777>)

## v0.1.0 - 2022-11-07

### Added

* Upload build artifacts to Computer Surgery Nix binary cache (<https://or.computer.surgery/charles/engage/-/commit/8773d4d932befa035707d01baa5b3cb902b6fb76>)
* Add a ton of tests (too many commits to link)

### Changed

* **BREAKING:** Disallow certain group names for forward compatibility (<https://or.computer.surgery/charles/engage/-/commit/066b2e38e8ee3fe5d777a9147abde92268bd795e>)
* **BREAKING:** `engage just <GROUP> [TASK]` now runs all dependencies (<https://or.computer.surgery/charles/engage/-/commit/acda3941c7a38da6ea0efb4f6f23bff29595e119>)
* Allow `engage self dot` to take `<GROUP> [TASK]` arguments to show a subgraph
  for the given target (<https://or.computer.surgery/charles/engage/-/commit/acda3941c7a38da6ea0efb4f6f23bff29595e119>)
* Make error messages a little prettier (<https://or.computer.surgery/charles/engage/-/commit/257e49eb3d5806427e3bb217e2bcb5aad982aa08>)
* Allow `engage self dot` even if there are cycles (<https://or.computer.surgery/charles/engage/-/commit/f6ffbc2307956eea18cbdbc70b6584c1016a12c6>)
* Print out the status at the end to make it easier to see (<https://or.computer.surgery/charles/engage/-/commit/37a6b9fdf76c19a6d6364839ac532a0eeeaa2a00>)
* Print out the group and name of the failing task, if any (<https://or.computer.surgery/charles/engage/-/commit/c3df8f615904d702f18466a8123e032266d4a3a6>)
* Print errors to `stderr` instead of `stdout` (<https://or.computer.surgery/charles/engage/-/commit/9c5dfaf7b16d95e1c9d645fa8f5cdb4149db097d>)
* Rework output while running the graph to be more functional and accessible (<https://or.computer.surgery/charles/engage/-/commit/8880003f969abe8604afb87a058764b4bdfba275>)

### Fixed

* **BREAKING:** Groups can no longer depend on groups that don't exist (<https://or.computer.surgery/charles/engage/-/commit/69219ece890d8084415aacb01c8339f43fe80e26>)
* Fix deadlock in an unusual situation (<https://or.computer.surgery/charles/engage/-/commit/acee5fe9457cfd9c231e46fae08657b32ba74e2b>)
* Prevent deadlock in an unusual situation (<https://or.computer.surgery/charles/engage/-/commit/295e3b66ac53b7c63c4f7f8ff08019ac779747e2>)
* Improve graph in unusual self-loop situation (<https://or.computer.surgery/charles/engage/-/commit/a9f1a273f8205908107635f26db3fa78e60a3f36>)
* Improve UX when `interpreter` is given an empty list (<https://or.computer.surgery/charles/engage/-/commit/72e89a3924f686732f2c7ac4faa768ff47c6af48>)
* Improve UX of some error messages (<https://or.computer.surgery/charles/engage/-/commit/c663bb63227b3bb95c6c7a9eb35a1ceddf95bd8f>)
* Greatly improve error messages for dependency cycle issues (<https://or.computer.surgery/charles/engage/-/commit/f6ffbc2307956eea18cbdbc70b6584c1016a12c6>)

## v0.1.0-alpha.2 - 2022-10-09

### Changed

* **BREAKING:** Rename `task.cmd` to `task.script` in `engage.toml` (<https://or.computer.surgery/charles/engage/-/commit/b403dbe12364f593b50d72949abc3bc61bce8953>)
* Upgrade `clap` to `^4` (<https://or.computer.surgery/charles/engage/-/commit/b0f7ac788c9505afbefd18f9cf5040ba831b77df>)

### Fixed

* Reset output style at the beginning of each line (first contribution; <https://or.computer.surgery/charles/engage/-/merge_requests/1>)

## v0.1.0-alpha.1 - 2022-09-17

First release! No changes to report.
