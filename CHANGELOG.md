# Changelog

## Unreleased

### Features

* **BREAKING:** Disallow certain group names for forward compatibility (<https://or.computer.surgery/charles/engage/-/commit/066b2e38e8ee3fe5d777a9147abde92268bd795e>)
* Make error messages a little prettier (<https://or.computer.surgery/charles/engage/-/commit/257e49eb3d5806427e3bb217e2bcb5aad982aa08>)
* Upload build artifacts to Computer Surgery Nix binary cache (<https://or.computer.surgery/charles/engage/-/commit/8773d4d932befa035707d01baa5b3cb902b6fb76>)
* Greatly improve error messages for dependency cycle issues (<https://or.computer.surgery/charles/engage/-/commit/f6ffbc2307956eea18cbdbc70b6584c1016a12c6>)
* Allow `engage self dot` even if there are cycles (<https://or.computer.surgery/charles/engage/-/commit/f6ffbc2307956eea18cbdbc70b6584c1016a12c6>)
* Add a ton of tests (too many commits to link)

### Fixes

* Print errors to `stderr` instead of `stdout` (<https://or.computer.surgery/charles/engage/-/commit/9c5dfaf7b16d95e1c9d645fa8f5cdb4149db097d>)
* Fix deadlock in an unusual situation (<https://or.computer.surgery/charles/engage/-/commit/acee5fe9457cfd9c231e46fae08657b32ba74e2b>)
* Prevent deadlock in an unusual situation (<https://or.computer.surgery/charles/engage/-/commit/295e3b66ac53b7c63c4f7f8ff08019ac779747e2>)

## v0.1.0-alpha.2 - 2022-10-09

### Features

* **BREAKING:** Rename `task.cmd` to `task.script` in `engage.toml` (<https://or.computer.surgery/charles/engage/-/commit/b403dbe12364f593b50d72949abc3bc61bce8953>)
* Upgrade `clap` to `^4` (<https://or.computer.surgery/charles/engage/-/commit/b0f7ac788c9505afbefd18f9cf5040ba831b77df>)

### Fixes

* Reset output style at the beginning of each line (first contribution; <https://or.computer.surgery/charles/engage/-/merge_requests/1>)

## v0.1.0-alpha.1 - 2022-09-17

First release! No changes to report.
