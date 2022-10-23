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

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use assert_cmd::{assert::OutputAssertExt, cargo::CommandCargoExt};
use engage::TASK_GROUP_NAME_SEPARATOR;
use path_macro::path;
use predicates::{self as p, prelude::PredicateBooleanExt};
use tempfile::tempdir;

/// Name used for a predicates context that describes the test
static DESCRIPTION: &str = "description";

type TestError = Box<dyn std::error::Error>;
type TestResult = Result<(), TestError>;

/// Try to run the binary and get its output
fn run(args: &[&str], engage_file: Option<&str>) -> Result<Output, TestError> {
    let td = tempdir()?;

    if let Some(engage_file) = engage_file {
        fs::copy(
            path!("tests/fixtures" / format!("{engage_file}.toml")),
            path!(td / "engage.toml"),
        )?;
    }

    Command::cargo_bin("engage")?
        .current_dir(&td)
        .args(args)
        .output()
        .map_err(Into::into)
}

/// Stolen from <https://insta.rs/docs/patterns/#rstest>
macro_rules! set_snapshot_suffix {
    ($($expr:expr),*) => {
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_suffix(format!($($expr,)*));
        let _guard = settings.bind_to_scope();
    }
}

/// Create a snapshot test
///
/// The arguments are:
///
/// * Function/test name (by default this is also the filename used from
///   `tests/fixtures`)
/// * Description of the intended behavior
/// * Optional arguments to the binary
/// * Optional alternate file, as `Option<&str>`
/// * Optional alternate assertion, as a path; (by default this is
///   [`insta::assert_debug_snapshot`](insta::assert_debug_snapshot))
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
        // Default to debug due to printing colors
        make_snapshot_test!(
            $name,
            $description,
            $args,
            $file,
            insta::assert_debug_snapshot
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

            let stdout = String::from_utf8(output.stdout)?;
            let stderr = String::from_utf8(output.stderr)?;
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
    no_file,
    "should exit with an error saying no engage file was found",
    [],
    None,
);

make_snapshot_test!(minimal, "should exit successfully after doing nothing");

make_snapshot_test!(
    one_task_implicit_group,
    "should exit sucessfully and implicitly create a group from the task",
);

make_snapshot_test!(
    serial_tasks,
    "should exit successfully after running a handful of tasks in serially",
);

make_snapshot_test!(
    group_dependency_cycle,
    "should exit with an error about dependency cycles"
);

make_snapshot_test!(
    groups_dependency_cycle,
    "should exit with an error about dependency cycles"
);

make_snapshot_test!(
    task_dependency_cycle,
    "should exit with an error about dependency cycles, in particular about \
     self-loops",
);

make_snapshot_test!(
    tasks_dependency_cycle,
    "should exit with an error about dependency cycles"
);

make_snapshot_test!(
    illegal_group_name_all,
    "should exit with an error about illegal group names"
);

make_snapshot_test!(
    illegal_group_name_help,
    "should exit with an error about illegal group names"
);

make_snapshot_test!(
    illegal_group_name_just,
    "should exit with an error about illegal group names"
);

make_snapshot_test!(
    illegal_group_name_self,
    "should exit with an error about illegal group names"
);

make_snapshot_test!(
    bubble_up_erroneous_exit_code,
    "should bubble up the exit code of a failing task"
);

make_snapshot_test!(
    ignored_nonzero_task_exit_status,
    "should still succeed because the nonzero exit status was ignored"
);

make_snapshot_test!(
    four_tasks_two_groups_graph,
    "should exit sucessfully after deterministically printing a graphviz dot \
     representation of the engage file",
    ["self", "dot"],
    Some("four_tasks_two_groups"),
    insta::assert_display_snapshot,
);

make_snapshot_test!(
    four_tasks_two_groups_graph_with_deps,
    "should exit successfully after deterministically printing a graphviz dot \
     representation of the engage file",
    ["self", "dot"],
    Some("four_tasks_two_groups_with_deps"),
    insta::assert_display_snapshot,
);

make_snapshot_test!(
    four_tasks_two_groups_list,
    "should exit successfully after deterministically printing a textual \
     representation of the engage file",
    ["self", "list"],
    Some("four_tasks_two_groups"),
    insta::assert_display_snapshot,
);

make_snapshot_test!(
    four_tasks_two_groups_list_with_deps,
    "should exit successfully after deterministically printing a textual \
     representation of the engage file",
    ["self", "list"],
    Some("four_tasks_two_groups_with_deps"),
    insta::assert_display_snapshot,
);

make_snapshot_test!(
    run_specific_task,
    "should exit successfully after running only \"task a\" from the \"group \
     a\" group",
    ["just", "group a", "task a"],
    Some("four_tasks_two_groups"),
);

make_snapshot_test!(
    run_specific_task_with_deps,
    "should exit successfully after running only \"task a\" from the \"group \
     a\" group",
    ["just", "group a", "task a"],
    Some("four_tasks_two_groups_with_deps"),
);

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
