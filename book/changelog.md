# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog][keep_a_changelog], and this project
adheres to [SemVer][semver].

[keep_a_changelog]: https://keepachangelog.com/en/1.0.0/
[semver]: https://semver.org/spec/v2.0.0.html

<!--

Changelog sections must appear in the following order if they appear for a
particular version so that attention can be drawn to the important parts:

1. Security
2. Removed
3. Deprecated
4. Changed
5. Fixed
6. Added

Entries within each section should be sorted by merge order. If multiple changes
result in a single entry, choose the merge order of the first or last change.
The first sentence of each entry should be phrased to complete the sentence,
"this release will [...]".

-->

## Unreleased

### Removed

1. **BREAKING:** Remove the concept of groups.
   ([!10](https://gitlab.computer.surgery/charles/engage/-/merge_requests/10))
2. **BREAKING:** Remove the `ignore` task option. It is seldom useful and can be
   implemented outside of Engage when it's truly necessary.
   ([!13](https://gitlab.computer.surgery/charles/engage/-/merge_requests/13))
3. **BREAKING:** Remove the `interpreter` global option as it is no longer
   necessary.
   ([!15](https://gitlab.computer.surgery/charles/engage/-/merge_requests/15))
4. **BREAKING:** Remove support for non-Unix platforms. Non-Unix platforms may
   become supported in the future.
   ([!29](https://gitlab.computer.surgery/charles/engage/-/merge_requests/29))

### Changed

1. **BREAKING:** Replace `[[task]]` and the `name` field with `[task.<name>]`.
   ([!11](https://gitlab.computer.surgery/charles/engage/-/merge_requests/11))
2. **BREAKING:** Replace `[task.<name>]` with `[tasks.<name>]` (note the new
   `s`).
   ([!11](https://gitlab.computer.surgery/charles/engage/-/merge_requests/11))
3. Deduplicate elements in the `depends` list. Duplicates are not rejected, but
   they are ignored while constructing the dependency graph, so e.g. they will
   no longer show up in `engage dot`.
   ([!11](https://gitlab.computer.surgery/charles/engage/-/merge_requests/11))
4. **BREAKING:** Rename `depends` to `after`.
   ([!11](https://gitlab.computer.surgery/charles/engage/-/merge_requests/11))
5. **BREAKING:** Replace the `script` task option with `command`, which takes a
   list of strings rather than a single string.
   ([!15](https://gitlab.computer.surgery/charles/engage/-/merge_requests/15))
6. **BREAKING:** In task names, only `[a-z0-9-]+` is permitted, and `-` cannot
   appear as the first character.
   ([!18](https://gitlab.computer.surgery/charles/engage/-/merge_requests/18))
7. **BREAKING:** Unknown fields in Engage files are now rejected.
   ([!19](https://gitlab.computer.surgery/charles/engage/-/merge_requests/19))
8. Improve error messages when attempting to run tasks.
   ([!21](https://gitlab.computer.surgery/charles/engage/-/merge_requests/21))
9. Improve the formatting of errors. The new format is much more likely to work
   well with screen readers, and can include more information than just the
   error message, such as suggestions for resolving the error.
   ([!21](https://gitlab.computer.surgery/charles/engage/-/merge_requests/21))

### Fixed

1. No longer block the starting of tasks on other tasks that they do not declare
   a direct or transitive dependency on.
   ([#9](https://gitlab.computer.surgery/charles/engage/-/issues/9),
   [!24](https://gitlab.computer.surgery/charles/engage/-/merge_requests/24),
   [!26](https://gitlab.computer.surgery/charles/engage/-/merge_requests/26))

### Added

1. Add the `before` task option, which requires that the task in question run
   before the tasks given to this option.
   ([!12](https://gitlab.computer.surgery/charles/engage/-/merge_requests/12))
2. Add the `environment` task option, which allows configuring environment
   variables on a per-task basis.
   ([!14](https://gitlab.computer.surgery/charles/engage/-/merge_requests/14),
   [!16](https://gitlab.computer.surgery/charles/engage/-/merge_requests/16))
3. Add the `-l`/`--log-format` CLI option for choosing alternate log formats.
   ([!27](https://gitlab.computer.surgery/charles/engage/-/merge_requests/27),
   [!28](https://gitlab.computer.surgery/charles/engage/-/merge_requests/28))
4. Add a `ctrl`+`c`/`SIGINT` handler which prevents new tasks from starting,
   cancels active tasks, and waits for them to exit.
   ([!29](https://gitlab.computer.surgery/charles/engage/-/merge_requests/29),
   [!32](https://gitlab.computer.surgery/charles/engage/-/merge_requests/32))

## v0.2.1 - 2025-09-08

### Changed

1. Greatly improve error messages in some cases, primarily task failures and
   dependency cycles.
   ([!4](https://gitlab.computer.surgery/charles/engage/-/merge_requests/4))

### Fixed

1. Only load the Engage file when necessary.
   ([2f394ad](https://gitlab.computer.surgery/charles/engage/-/commit/2f394adbecd54d66d8c4df8b8b6021f7faf2e09b))

### Added

1. Add more thorough documentation in the form of a book.
   (Too many commits to link.)
2. The long help CLI output links to a local copy of the book, if packaged to do
   so.
   ([!2](https://gitlab.computer.surgery/charles/engage/-/merge_requests/2))

## v0.2.0 - 2023-09-19

### Changed

1. **BREAKING:** Always exit with a status of `1` when a task fails instead of
   trying to exit with the same status code as the task.
   ([c18357e](https://gitlab.computer.surgery/charles/engage/-/commit/c18357eea2bba8963b0747a667efcff85039dea3))
2. **BREAKING:** Exit with a status of `2` for errors other than tasks failing.
   ([a991d12](https://gitlab.computer.surgery/charles/engage/-/commit/a991d12777d297f6d4d4c756e7eb349d2ff39720))
3. **BREAKING:** Move subcommands from under `self` to the top level and remove
   the `self` subcommand. For example, you'd now use `engage dot` instead of
   `engage self dot`.
   ([e537e9d](https://gitlab.computer.surgery/charles/engage/-/commit/e537e9dda930fca1a6864c27c3367bbca399c14e))
4. Remove all restrictions on group names.
   ([0850054](https://gitlab.computer.surgery/charles/engage/-/commit/0850054aececd133a84c8ad8a0dfb480bf79f035))

### Fixed

1. Report all failed tasks instead of just the first one.
   ([b9fcdcd](https://gitlab.computer.surgery/charles/engage/-/commit/b9fcdcdf5d0fdcae426839bb36b6a22cd5b3650f))

## v0.1.3 - 2023-09-10

### Removed

1. Remove the Nix binary cache.
   ([066e97c](https://gitlab.computer.surgery/charles/engage/-/commit/066e97cd8116ac277638ca3933abed62259711ac))

### Fixed

1. Wait for all started tasks to complete before terminating.
   ([d98c6cc](https://gitlab.computer.surgery/charles/engage/-/commit/d98c6cc552b979256a85cd79de898e34963fbdc8))

## v0.1.2 - 2023-02-10

### Changed

1. Improve the behavior summary.
   ([7be43e4](https://gitlab.computer.surgery/charles/engage/-/commit/7be43e4e11ca50dc8415f184865eb0a65d585375))

### Added

1. Add a `--jobs`/`-j` option to limit parallelism.
   ([da1cc61](https://gitlab.computer.surgery/charles/engage/-/commit/da1cc613e0b535fb7499a441f40011939452fda5))

## v0.1.1 - 2023-01-28

### Added

1. Add a subcommand to generate shell completions.
   ([48c925b](https://gitlab.computer.surgery/charles/engage/-/commit/48c925bb0c7ab216c72e7ab47891a2a6c26cc777))

## v0.1.0 - 2022-11-07

### Changed

1. **BREAKING:** Disallow certain group names for forward compatibility.
   ([066b2e3](https://gitlab.computer.surgery/charles/engage/-/commit/066b2e38e8ee3fe5d777a9147abde92268bd795e))
2. **BREAKING:** Make `engage just <GROUP> [TASK]` run all dependencies.
   ([acda394](https://gitlab.computer.surgery/charles/engage/-/commit/acda3941c7a38da6ea0efb4f6f23bff29595e119))
3. Allow `engage self dot` to take `<GROUP> [TASK]` arguments to show a subgraph
   for the given target.
   ([acda394](https://gitlab.computer.surgery/charles/engage/-/commit/acda3941c7a38da6ea0efb4f6f23bff29595e119))
4. Make error messages a little prettier.
   ([257e49e](https://gitlab.computer.surgery/charles/engage/-/commit/257e49eb3d5806427e3bb217e2bcb5aad982aa08))
5. Allow `engage self dot` even if there are cycles.
   ([f6ffbc2](https://gitlab.computer.surgery/charles/engage/-/commit/f6ffbc2307956eea18cbdbc70b6584c1016a12c6))
6. Print out the status at the end to make it easier to see.
   ([37a6b9f](https://gitlab.computer.surgery/charles/engage/-/commit/37a6b9fdf76c19a6d6364839ac532a0eeeaa2a00))
7. Print out the group and name of the failing task, if any.
   ([c3df8f6](https://gitlab.computer.surgery/charles/engage/-/commit/c3df8f615904d702f18466a8123e032266d4a3a6))
8. Print errors to `stderr` instead of `stdout`.
   ([9c5dfaf](https://gitlab.computer.surgery/charles/engage/-/commit/9c5dfaf7b16d95e1c9d645fa8f5cdb4149db097d))
9. Rework output while running the graph to be more functional and accessible.
   ([8880003](https://gitlab.computer.surgery/charles/engage/-/commit/8880003f969abe8604afb87a058764b4bdfba275))

### Fixed

1. **BREAKING:** Prevent groups from depending on nonexistent groups.
   ([69219ec](https://gitlab.computer.surgery/charles/engage/-/commit/69219ece890d8084415aacb01c8339f43fe80e26))
2. Fix a deadlock in an unusual situation.
   ([acee5fe](https://gitlab.computer.surgery/charles/engage/-/commit/acee5fe9457cfd9c231e46fae08657b32ba74e2b))
3. Prevent a deadlock in an unusual situation.
   ([295e3b6](https://gitlab.computer.surgery/charles/engage/-/commit/295e3b66ac53b7c63c4f7f8ff08019ac779747e2))
4. Improve the graph in unusual self-loop situation.
   ([a9f1a27](https://gitlab.computer.surgery/charles/engage/-/commit/a9f1a273f8205908107635f26db3fa78e60a3f36))
5. Improve UX when `interpreter` is given an empty list.
   ([72e89a3](https://gitlab.computer.surgery/charles/engage/-/commit/72e89a3924f686732f2c7ac4faa768ff47c6af48))
6. Improve UX of some error messages.
   ([c663bb6](https://gitlab.computer.surgery/charles/engage/-/commit/c663bb63227b3bb95c6c7a9eb35a1ceddf95bd8f))
7. Greatly improve error messages for dependency cycle issues.
   ([f6ffbc2](https://gitlab.computer.surgery/charles/engage/-/commit/f6ffbc2307956eea18cbdbc70b6584c1016a12c6))

### Added

1. Upload build artifacts to Computer Surgery Nix binary cache.
   ([8773d4d](https://gitlab.computer.surgery/charles/engage/-/commit/8773d4d932befa035707d01baa5b3cb902b6fb76))

## v0.1.0-alpha.2 - 2022-10-09

### Changed

1. **BREAKING:** Rename `task.cmd` to `task.script` in `engage.toml`.
   ([b403dbe](https://gitlab.computer.surgery/charles/engage/-/commit/b403dbe12364f593b50d72949abc3bc61bce8953))
2. Change CLI UX due to a dependency update.
   ([b0f7ac7](https://gitlab.computer.surgery/charles/engage/-/commit/b0f7ac788c9505afbefd18f9cf5040ba831b77df))

### Fixed

1. Reset output style at the beginning of each line.
   ([!1](https://gitlab.computer.surgery/charles/engage/-/merge_requests/1))

## v0.1.0-alpha.1 - 2022-09-17

Initial release.
