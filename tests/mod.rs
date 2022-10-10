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

use std::{fs, process::Command};

use assert_cmd::{assert::OutputAssertExt, cargo::CommandCargoExt};
use crossterm::{
    execute,
    style::{Attribute, Print, SetAttribute, Stylize},
};
use engage::{OUTPUT_SEPARATOR, TASK_GROUP_NAME_SEPARATOR};
use path_macro::path;
use predicates::{self as p, boolean::PredicateBooleanExt};
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
        .stderr(
            p::constant::always()
                .and(p::str::starts_with("error:"))
                .and(p::str::contains("engage.toml"))
                .and(p::str::contains("not found")),
        )
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
