#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![warn(clippy::as_conversions)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::empty_structs_with_brackets)]
#![warn(clippy::get_unwrap)]
#![warn(clippy::if_then_some_else_none)]
#![warn(clippy::let_underscore_must_use)]
#![warn(clippy::map_err_ignore)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::negative_feature_names)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::rc_mutex)]
#![warn(clippy::redundant_feature_names)]
#![warn(clippy::rest_pat_in_fully_bound_structs)]
#![warn(clippy::str_to_string)]
#![warn(clippy::string_add)]
#![warn(clippy::string_slice)]
#![warn(clippy::string_to_string)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(clippy::unseparated_literal_suffix)]
#![warn(clippy::wildcard_dependencies)]

use std::{borrow::Cow, fs, path::Path, process::Command};

use assert_cmd::{assert::OutputAssertExt, cargo::CommandCargoExt};
use crossterm::{
    execute,
    style::{Attribute, Print, SetAttribute, Stylize},
};
use engage::{error, OUTPUT_SEPARATOR, TASK_GROUP_NAME_SEPARATOR};
use indoc::indoc;
use path_macro::path;
use predicates::{self as p, prelude::PredicateBooleanExt};
use tempfile::tempdir;

/// Name used for a predicates context that describes the test
static DESCRIPTION: &str = "description";

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn no_engage_file() -> TestResult {
    let td = tempdir()?;

    Command::cargo_bin("engage")
        .unwrap()
        .current_dir(&td)
        .assert()
        .append_context(
            DESCRIPTION,
            "no engage file should be found, and that should be an error",
        )
        .stdout(p::str::is_empty())
        .stderr(p::str::diff(error::format_cli(
            "engage.toml not found in the current directory or its ancestors",
        )))
        .failure();

    Ok(())
}

#[test]
fn minimal_engage_file() -> TestResult {
    let td = tempdir()?;

    fs::copy("tests/fixtures/minimal.toml", path!(td / "engage.toml"))?;

    Command::cargo_bin("engage")
        .unwrap()
        .current_dir(&td)
        .assert()
        .append_context(DESCRIPTION, "should succeed but do nothing")
        .stdout(p::str::is_empty())
        .stderr(p::str::is_empty())
        .success();

    Ok(())
}

#[test]
fn one_task_implicit_group() -> TestResult {
    let td = tempdir()?;

    fs::copy("tests/fixtures/one_task.toml", path!(td / "engage.toml"))?;

    let mut buf = Vec::new();

    execute!(
        buf,
        SetAttribute(Attribute::Reset),
        Print("group"),
        Print(TASK_GROUP_NAME_SEPARATOR),
        Print("task "),
        Print(OUTPUT_SEPARATOR.green()),
        Print(" hello world\n"),
    )?;

    Command::cargo_bin("engage")
        .unwrap()
        .current_dir(&td)
        .assert()
        .append_context(DESCRIPTION, "should succeed, printing hello world")
        .stdout(p::str::diff(String::from_utf8(buf)?))
        .stderr(p::str::is_empty())
        .success();

    Ok(())
}

#[test]
fn groups_dependency_cycle() -> TestResult {
    let td = tempdir()?;

    fs::copy(
        "tests/fixtures/groups_dependency_cycle.toml",
        path!(td / "engage.toml"),
    )?;

    Command::cargo_bin("engage")
        .unwrap()
        .current_dir(&td)
        .assert()
        .append_context(DESCRIPTION, "should fail due to dependency cycles")
        .stdout(p::str::is_empty())
        .stderr(p::str::diff(error::format_cli("dependency cycle detected")))
        .code(1)
        .failure();

    Ok(())
}

#[test]
fn serial_tasks() -> TestResult {
    let td = tempdir()?;

    fs::copy("tests/fixtures/serial_tasks.toml", path!(td / "engage.toml"))?;

    let mut buf = Vec::new();

    for task in ["a", "b", "c"] {
        execute!(
            buf,
            SetAttribute(Attribute::Reset),
            Print("group"),
            Print(TASK_GROUP_NAME_SEPARATOR),
            Print(task),
            Print(' '),
            Print(OUTPUT_SEPARATOR.green()),
            Print(' '),
            Print(task),
            Print('\n'),
        )?;
    }

    Command::cargo_bin("engage")
        .unwrap()
        .current_dir(&td)
        .assert()
        .append_context(
            DESCRIPTION,
            "should successfully run all tasks in a deterministic order",
        )
        .stdout(p::str::diff(String::from_utf8(buf)?))
        .stderr(p::str::is_empty())
        .success();

    Ok(())
}

#[test]
fn run_specific_group() -> TestResult {
    run_specific_group_inner("tests/fixtures/four_tasks_two_groups.toml")
}

#[test]
fn run_specific_group_with_deps() -> TestResult {
    run_specific_group_inner(
        "tests/fixtures/four_tasks_two_groups_with_deps.toml",
    )
}

fn run_specific_group_inner<P>(engage_file: P) -> TestResult
where
    P: AsRef<Path>,
{
    let td = tempdir()?;

    fs::copy(engage_file, path!(td / "engage.toml"))?;

    Command::cargo_bin("engage")
        .unwrap()
        .arg("just")
        .arg("group a")
        .current_dir(&td)
        .assert()
        .append_context(
            DESCRIPTION,
            "should successfully run only tasks in the \"group a\" group",
        )
        .stdout(
            p::constant::always()
                .and(p::str::contains(format!(
                    "group a{}task a",
                    TASK_GROUP_NAME_SEPARATOR
                )))
                .and(p::str::contains(format!(
                    "group a{}task b",
                    TASK_GROUP_NAME_SEPARATOR
                )))
                .and(
                    p::str::contains(format!(
                        "group b{}task a",
                        TASK_GROUP_NAME_SEPARATOR
                    ))
                    .not(),
                )
                .and(
                    p::str::contains(format!(
                        "group b{}task b",
                        TASK_GROUP_NAME_SEPARATOR
                    ))
                    .not(),
                ),
        )
        .stderr(p::str::is_empty())
        .success();

    Ok(())
}

#[test]
fn run_specific_task() -> TestResult {
    run_specific_task_inner("tests/fixtures/four_tasks_two_groups.toml")
}

#[test]
fn run_specific_task_with_deps() -> TestResult {
    run_specific_task_inner(
        "tests/fixtures/four_tasks_two_groups_with_deps.toml",
    )
}

fn run_specific_task_inner<P>(engage_file: P) -> TestResult
where
    P: AsRef<Path>,
{
    let td = tempdir()?;

    fs::copy(engage_file, path!(td / "engage.toml"))?;

    Command::cargo_bin("engage")
        .unwrap()
        .arg("just")
        .arg("group a")
        .arg("task a")
        .current_dir(&td)
        .assert()
        .append_context(
            DESCRIPTION,
            "should successfully run only \"task a\" in the \"group a\" group",
        )
        .stdout(
            p::constant::always()
                .and(p::str::contains(format!(
                    "group a{}task a",
                    TASK_GROUP_NAME_SEPARATOR
                )))
                .and(
                    p::str::contains(format!(
                        "group a{}task b",
                        TASK_GROUP_NAME_SEPARATOR
                    ))
                    .not(),
                )
                .and(
                    p::str::contains(format!(
                        "group b{}task a",
                        TASK_GROUP_NAME_SEPARATOR
                    ))
                    .not(),
                )
                .and(
                    p::str::contains(format!(
                        "group b{}task b",
                        TASK_GROUP_NAME_SEPARATOR
                    ))
                    .not(),
                ),
        )
        .stderr(p::str::is_empty())
        .success();

    Ok(())
}

#[test]
fn four_tasks_two_groups_graph() -> TestResult {
    four_tasks_two_groups_with_deps_graph_inner(
        "tests/fixtures/four_tasks_two_groups.toml",
        indoc!(
            r#"
                digraph {
                    0 [ label = "group start: group a" ]
                    1 [ label = "group end" ]
                    2 [ label = "task: task a" ]
                    3 [ label = "task: task b" ]
                    4 [ label = "group start: group b" ]
                    5 [ label = "group end" ]
                    6 [ label = "task: task a" ]
                    7 [ label = "task: task b" ]
                    0 -> 2 [ label = "1" ]
                    2 -> 1 [ label = "1" ]
                    0 -> 3 [ label = "1" ]
                    3 -> 1 [ label = "1" ]
                    4 -> 6 [ label = "1" ]
                    6 -> 5 [ label = "1" ]
                    4 -> 7 [ label = "1" ]
                    7 -> 5 [ label = "1" ]
                }
            "#
        ),
    )
}

#[test]
fn four_tasks_two_groups_with_deps_graph() -> TestResult {
    four_tasks_two_groups_with_deps_graph_inner(
        "tests/fixtures/four_tasks_two_groups_with_deps.toml",
        indoc!(
            r#"
                digraph {
                    0 [ label = "group start: group b" ]
                    1 [ label = "group end" ]
                    2 [ label = "task: task a" ]
                    3 [ label = "task: task b" ]
                    4 [ label = "group start: group a" ]
                    5 [ label = "group end" ]
                    6 [ label = "task: task a" ]
                    7 [ label = "task: task b" ]
                    0 -> 2 [ label = "1" ]
                    3 -> 1 [ label = "1" ]
                    2 -> 3 [ label = "1" ]
                    4 -> 6 [ label = "1" ]
                    7 -> 5 [ label = "1" ]
                    6 -> 7 [ label = "1" ]
                    5 -> 0 [ label = "1" ]
                }
            "#
        ),
    )
}

fn four_tasks_two_groups_with_deps_graph_inner<P, S>(
    engage_file: P,
    expected: S,
) -> TestResult
where
    P: AsRef<Path>,
    S: Into<Cow<'static, str>>,
{
    let td = tempdir()?;

    fs::copy(engage_file, path!(td / "engage.toml"))?;

    Command::cargo_bin("engage")
        .unwrap()
        .arg("self")
        .arg("dot")
        .current_dir(&td)
        .assert()
        .append_context(
            DESCRIPTION,
            "should deterministically print the graphviz representation of \
             the engage file",
        )
        .stdout(p::str::diff(expected))
        .stderr(p::str::is_empty())
        .success();

    Ok(())
}

#[test]
fn four_tasks_two_groups_list() -> TestResult {
    four_tasks_two_groups_list_inner(
        "tests/fixtures/four_tasks_two_groups.toml",
    )
}

#[test]
fn four_tasks_two_groups_list_with_deps() -> TestResult {
    four_tasks_two_groups_list_inner(
        "tests/fixtures/four_tasks_two_groups_with_deps.toml",
    )
}

fn four_tasks_two_groups_list_inner<P>(engage_file: P) -> TestResult
where
    P: AsRef<Path>,
{
    let td = tempdir()?;

    fs::copy(engage_file, path!(td / "engage.toml"))?;

    Command::cargo_bin("engage")
        .unwrap()
        .arg("self")
        .arg("list")
        .current_dir(&td)
        .assert()
        .append_context(
            DESCRIPTION,
            "should deterministically print a textual representation of the \
             engage file",
        )
        .stdout(p::str::diff(indoc!(
            r#"
                group a:
                    task a
                    task b

                group b:
                    task a
                    task b
            "#
        )))
        .stderr(p::str::is_empty())
        .success();

    Ok(())
}
