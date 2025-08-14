//! Integration tests.

// https://github.com/rust-lang/rust-clippy/issues/11024
#![allow(clippy::tests_outside_test_module)]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use assert_cmd::{assert::OutputAssertExt as _, cargo::CommandCargoExt as _};
use path_macro::path;
use predicates::{self as p, prelude::PredicateBooleanExt as _};
use strip_ansi_escapes::strip;
use tempfile::tempdir;

/// Name used for a predicates context that describes the test.
static DESCRIPTION: &str = "description";

type TestError = Box<dyn std::error::Error>;
type TestResult = Result<(), TestError>;

/// Try to run the binary and get its output.
fn run(args: &[&str], file: Option<&str>) -> Result<Output, TestError> {
    let td = tempdir()?;

    if let Some(file) = file {
        fs::copy(
            path!("tests/integrations/fixtures" / format!("{file}.toml")),
            path!(td / "engage.toml"),
        )?;
    }

    Command::cargo_bin("engage")?
        .current_dir(&td)
        .args(args)
        .output()
        .map_err(Into::into)
}

/// Stolen from <https://insta.rs/docs/patterns/#rstest>.
macro_rules! set_snapshot_suffix {
    ($($expr:expr),*) => {
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_suffix(format!($($expr,)*));
        let _guard = settings.bind_to_scope();
    }
}

/// Create a snapshot test.
///
/// The arguments are:
///
/// * Function/test name (by default this is also the filename used from
///   `tests/integrations/fixtures`).
/// * Description of the intended behavior.
/// * Optional arguments to the binary.
/// * Optional alternate file, as `Option<&str>`.
/// * Optional alternate assertion, as a path. (By default, this is
///   [`insta::assert_debug_snapshot`](insta::assert_debug_snapshot))
#[expect(unused_macro_rules)]
macro_rules! make_snapshot_test {
    ($name:ident, $description:expr $(,)?) => {
        make_snapshot_test!($name, $description, [], Some(stringify!($name)));
    };

    ($name:ident, $description:expr, $args:expr, $(,)?) => {
        make_snapshot_test!(
            $name,
            $description,
            $args,
            Some(stringify!($name))
        );
    };

    ($name:ident, $description:expr, $args:expr, $file:expr $(,)?) => {
        // Default to debug due to printing colors.
        make_snapshot_test!(
            $name,
            $description,
            $args,
            $file,
            insta::assert_snapshot
        );
    };

    (
        $name:ident,
        $description:expr,
        $args:expr,
        $file:expr,
        $insta_assertion:path $(,)?
    ) => {
        #[test]
        fn $name() -> TestResult {
            let output = run(&$args, $file)?;

            let stdout = String::from_utf8(strip(output.stdout))?;
            let stderr = String::from_utf8(strip(output.stderr))?;
            let status_code = output.status.code();

            insta::with_settings!({
                description => $description,
                omit_expression => true,
            }, {
                set_snapshot_suffix!("stdout");
                $insta_assertion!(stdout);

                set_snapshot_suffix!("stderr");
                $insta_assertion!(stderr);

                set_snapshot_suffix!("status_code");
                insta::assert_debug_snapshot!(status_code);
            });

            Ok(())
        }
    };
}

make_snapshot_test!(
    long_help,
    "Should successfully print the long help and exit.",
    ["help"],
    None,
);

make_snapshot_test!(
    short_help,
    "Should successfully print the short help and exit.",
    ["-h"],
    None,
);

make_snapshot_test!(
    no_file,
    "Should exit with an error saying no engage file was found.",
    [],
    None,
);

make_snapshot_test!(minimal, "Should exit successfully after doing nothing.");

make_snapshot_test!(
    one_task_implicit_group,
    "Should exit sucessfully and implicitly create a group from the task.",
);

make_snapshot_test!(
    serial_tasks,
    "Should exit successfully after running a handful of tasks in serially.",
);

make_snapshot_test!(
    group_dependency_cycle,
    "Should exit with an error about dependency cycles."
);

make_snapshot_test!(
    groups_dependency_cycle,
    "Should exit with an error about dependency cycles."
);

make_snapshot_test!(
    task_dependency_cycle,
    "Should exit with an error about dependency cycles, in particular about \
     self-loops.",
);

make_snapshot_test!(
    tasks_dependency_cycle,
    "Should exit with an error about dependency cycles."
);

make_snapshot_test!(
    tasks_dependency_cycle_self_loop,
    "Should exit with an error about dependency cycles."
);

make_snapshot_test!(
    exit_code_task_nonzero_exit,
    "Should exit with a code indicating a task exited with a nonzero and \
     unignored exit code."
);

make_snapshot_test!(
    ignored_nonzero_task_exit_status,
    "Should still succeed because the nonzero exit status was ignored."
);

make_snapshot_test!(
    four_tasks_two_groups_graph,
    "Should exit sucessfully after deterministically printing a graphviz dot \
     representation of the engage file.",
    ["dot"],
    Some("four_tasks_two_groups"),
);

make_snapshot_test!(
    four_tasks_two_groups_graph_with_deps,
    "Should exit successfully after deterministically printing a graphviz dot \
     representation of the engage file.",
    ["dot"],
    Some("four_tasks_two_groups_with_deps"),
);

make_snapshot_test!(
    four_tasks_two_groups_group_subgraph_with_deps,
    "Should exit successfully after deterministically printing a graphviz dot \
     representation of the requested subgraph of the engage file.",
    ["dot", "group b"],
    Some("four_tasks_two_groups_with_deps"),
);

make_snapshot_test!(
    four_tasks_two_groups_task_subgraph_with_deps,
    "Should exit successfully after deterministically printing a graphviz dot \
     representation of the requested subgraph of the engage file.",
    ["dot", "group b", "task a"],
    Some("four_tasks_two_groups_with_deps"),
);

make_snapshot_test!(
    four_tasks_two_groups_list,
    "Should exit successfully after deterministically printing a textual \
     representation of the engage file.",
    ["list"],
    Some("four_tasks_two_groups"),
);

make_snapshot_test!(
    four_tasks_two_groups_list_with_deps,
    "Should exit successfully after deterministically printing a textual \
     representation of the engage file.",
    ["list"],
    Some("four_tasks_two_groups_with_deps"),
);

make_snapshot_test!(
    run_specific_task,
    "Should exit successfully after running only \"task b\" from the \"group \
     b\" group.",
    ["just", "group b", "task b"],
    Some("four_tasks_two_groups"),
);

make_snapshot_test!(
    run_specific_task_with_deps,
    "Should exit successfully after running only \"task b\" from the \"group \
     b\" group.",
    ["just", "group b", "task b"],
    Some("four_tasks_two_groups_with_deps"),
);

make_snapshot_test!(
    group_dependency_cycle_dot,
    "Should show the graphviz dot representation even though there are cycles.",
    ["dot"],
    Some("group_dependency_cycle"),
);

make_snapshot_test!(
    groups_dependency_cycle_dot,
    "Should show the graphviz dot representation even though there are cycles.",
    ["dot"],
    Some("groups_dependency_cycle"),
);

make_snapshot_test!(
    task_dependency_cycle_dot,
    "Should show the graphviz dot representation even though there are cycles.",
    ["dot"],
    Some("task_dependency_cycle"),
);

make_snapshot_test!(
    tasks_dependency_cycle_dot,
    "Should show the graphviz dot representation even though there are cycles.",
    ["dot"],
    Some("tasks_dependency_cycle"),
);

make_snapshot_test!(
    tasks_dependency_cycle_self_loop_dot,
    "Should show the graphviz dot representation even though there are cycles.",
    ["dot"],
    Some("tasks_dependency_cycle_self_loop"),
);

make_snapshot_test!(
    try_nonexistent_group,
    "Should exit with an error about the requested group not existing.",
    ["just", "doesntexist"],
    Some("minimal"),
);

make_snapshot_test!(
    try_nonexistent_task,
    "Should exit with an error about the requested task not existing.",
    ["just", "group", "doesntexist"],
    Some("one_task_implicit_group"),
);

make_snapshot_test!(
    try_nonexistent_both,
    "Should exit with an error about, at least, the requested group not \
     existing.",
    ["just", "doesnt", "exist"],
    Some("minimal"),
);

make_snapshot_test!(
    task_bad_dependency,
    "Should exit with an error about invalid task dependencies.",
);

make_snapshot_test!(
    group_bad_dependency,
    "Should exit with an error about invalid group dependencies.",
);

make_snapshot_test!(
    task_prints_to_stderr,
    "Should exit sucessfully after redirecting the task's output to stdout.",
);

make_snapshot_test!(
    empty_interpreter,
    "Should exit with an error about the interpreter being empty.",
);

make_snapshot_test!(
    interpreter_not_found,
    "Should exit with an error about the interpreter not being found.",
);

make_snapshot_test!(
    schema,
    "Should print the JSON Schema for the Engage file and exit.",
    ["schema"],
    None,
);

#[test]
fn run_specific_group() -> TestResult {
    run_specific_group_inner(
        "tests/integrations/fixtures/four_tasks_two_groups.toml",
    )
}

#[test]
fn run_specific_group_with_deps() -> TestResult {
    run_specific_group_inner(
        "tests/integrations/fixtures/four_tasks_two_groups_with_deps.toml",
    )
}

fn run_specific_group_inner<P>(file: P) -> TestResult
where
    P: AsRef<Path>,
{
    let td = tempdir()?;

    fs::copy(file, path!(td / "engage.toml"))?;

    Command::cargo_bin("engage")
        .expect("should be able to find the crate's binary")
        .arg("just")
        .arg("group a")
        .current_dir(&td)
        .assert()
        .append_context(
            DESCRIPTION,
            "Should successfully run only tasks in the \"group a\" group.",
        )
        .stdout(
            p::constant::always()
                .and(p::str::contains("group a::task a"))
                .and(p::str::contains("group a::task b"))
                .and(p::str::contains("group b::task a").not())
                .and(p::str::contains("group b::task b").not()),
        )
        .stderr(p::str::is_empty())
        .success();

    Ok(())
}

#[test]
fn alternate_file() -> TestResult {
    let td = tempdir()?;

    fs::copy(
        "tests/integrations/fixtures/minimal.toml",
        path!(td / "engage.toml"),
    )?;
    fs::copy(
        "tests/integrations/fixtures/one_task_implicit_group.toml",
        path!(td / "other.toml"),
    )?;

    let output = Command::cargo_bin("engage")?
        .current_dir(&td)
        .args(["-f", "other.toml"])
        .output()?;

    let stdout = String::from_utf8(strip(output.stdout))?;
    let stderr = String::from_utf8(strip(output.stderr))?;
    let status_code = output.status.code();

    insta::with_settings!({
        description => "Should successfully run the task in `other.toml`.",
        omit_expression => true,
    }, {
        set_snapshot_suffix!("stdout");
        insta::assert_snapshot!(stdout);

        set_snapshot_suffix!("stderr");
        insta::assert_snapshot!(stderr);

        set_snapshot_suffix!("status_code");
        insta::assert_debug_snapshot!(status_code);
    });

    Ok(())
}

#[test]
fn report_all_errors() -> TestResult {
    let output = run(&[], Some("report_all_errors"))?;

    let stdout = String::from_utf8(strip(output.stdout))?;
    let stderr = String::from_utf8(strip(output.stderr))?
        .replace("group::a", "[redacted task name]")
        .replace("group::b", "[redacted task name]")
        .replace("group::c", "[redacted task name]")
        .replace("exit status: 1", "[redacted exit status]")
        .replace("exit status: 2", "[redacted exit status]")
        .replace("exit status: 3", "[redacted exit status]");
    let status_code = output.status.code();

    insta::with_settings!({
        description => "Should display multiple task failures in a \
            well-formatted way.",
        omit_expression => true,
    }, {
        set_snapshot_suffix!("stdout");
        insta::assert_snapshot!(stdout);

        set_snapshot_suffix!("stderr");
        insta::assert_snapshot!(stderr);

        set_snapshot_suffix!("status_code");
        insta::assert_debug_snapshot!(status_code);
    });

    Ok(())
}
