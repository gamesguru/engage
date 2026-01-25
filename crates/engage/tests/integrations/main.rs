//! Integration tests.

// https://github.com/rust-lang/rust-clippy/issues/11024
#![allow(clippy::tests_outside_test_module)]

use std::{
    fs,
    process::{Command, Output, Stdio},
    time::Duration,
};

#[expect(deprecated, reason = "function is deprecated but macro is not")]
use assert_cmd::cargo::cargo_bin;
use nix::sys::signal::{Signal, kill};
use path_macro::path;
use strip_ansi_escapes::strip;
use tempfile::tempdir;
use tokio::{
    io::{AsyncRead, AsyncReadExt as _},
    time::timeout,
};
use util::ChildExt as _;

type TestError = Box<dyn std::error::Error>;
type TestResult = Result<(), TestError>;

/// Copies from the reader into the haystack, then checks it for the needle.
async fn is_needle_in_haystack<R>(
    needle: &[u8],
    haystack: &mut Vec<u8>,
    mut reader: R,
) -> Result<bool, std::io::Error>
where
    R: AsyncRead + Unpin,
{
    let mut buf = [0; 1024];

    // Search existing haystack first.
    if haystack.windows(needle.len()).any(|x| x == needle) {
        return Ok(true);
    }

    // Extend haystack with a read and then search it.
    loop {
        let n = reader.read(&mut buf).await?;

        if n == 0 {
            return Ok(false);
        }

        haystack.extend_from_slice(&buf[..n]);

        // Search in reverse since the needle will now appear due to a recent
        // read, if at all.
        if haystack.windows(needle.len()).rev().any(|x| x == needle) {
            return Ok(true);
        }
    }
}

/// Try to run the binary and get its output.
fn run(args: &[&str], file: Option<&str>) -> Result<Output, TestError> {
    let td = tempdir()?;

    if let Some(file) = file {
        fs::copy(
            path!("tests/integrations/fixtures" / format!("{file}.toml")),
            path!(td / "engage.toml"),
        )?;
    }

    Command::new(cargo_bin!("engage"))
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
    "Should exit with an error saying no Engage file was found.",
    [],
    None,
);

make_snapshot_test!(minimal, "Should exit successfully after doing nothing.");

make_snapshot_test!(
    one_process,
    "Should exit sucessfully after running a process."
);

make_snapshot_test!(
    serial_processes,
    "Should exit successfully after running a handful of processes serially.",
);

make_snapshot_test!(self_loop, "Should exit with an error about self-loops.");

make_snapshot_test!(
    dependency_cycle,
    "Should exit with an error about dependency cycles."
);

make_snapshot_test!(
    dependency_cycle_self_loop,
    "Should exit with an error about dependency cycles."
);

make_snapshot_test!(
    exit_code_process_nonzero_exit,
    "Should exit with a code indicating a process exited with an unsuccessful \
     exit code."
);

make_snapshot_test!(
    four_processes_graph,
    "Should exit sucessfully after deterministically printing a graphviz dot \
     representation of the Engage file.",
    ["dot"],
    Some("four_processes"),
);

make_snapshot_test!(
    four_processes_with_deps_graph,
    "Should exit successfully after deterministically printing a graphviz dot \
     representation of the Engage file.",
    ["dot"],
    Some("four_processes_with_deps"),
);

make_snapshot_test!(
    four_processes_with_deps_subgraph,
    "Should exit successfully after deterministically printing a graphviz dot \
     representation of the requested subgraph of the Engage file.",
    ["dot", "d"],
    Some("four_processes_with_deps"),
);

make_snapshot_test!(
    four_processes_list,
    "Should exit successfully after deterministically printing a textual \
     representation of the Engage file.",
    ["list"],
    Some("four_processes"),
);

make_snapshot_test!(
    four_processes_with_deps_list,
    "Should exit successfully after deterministically printing a textual \
     representation of the Engage file.",
    ["list"],
    Some("four_processes_with_deps"),
);

make_snapshot_test!(
    run_specific_process,
    "Should exit successfully after running only `d`.",
    ["just", "d"],
    Some("four_processes"),
);

make_snapshot_test!(
    run_specific_process_with_deps,
    "Should exit successfully after running `a` and `b`.",
    ["just", "b"],
    Some("four_processes_with_deps"),
);

make_snapshot_test!(
    self_loop_dot,
    "Should show the graphviz dot representation even though there are cycles.",
    ["dot"],
    Some("dependency_cycle"),
);

make_snapshot_test!(
    dependency_cycle_dot,
    "Should show the graphviz dot representation even though there are cycles.",
    ["dot"],
    Some("dependency_cycle"),
);

make_snapshot_test!(
    dependency_cycle_self_loop_dot,
    "Should show the graphviz dot representation even though there are cycles.",
    ["dot"],
    Some("dependency_cycle_self_loop"),
);

make_snapshot_test!(
    try_nonexistent_process,
    "Should exit with an error about the requested process not existing.",
    ["just", "doesntexist"],
    Some("minimal"),
);

make_snapshot_test!(
    missing_dependencies,
    "Should exit with an error about missing dependencies.",
);

make_snapshot_test!(
    process_prints_to_stderr,
    "Should exit sucessfully after redirecting the process' output to stdout.",
);

make_snapshot_test!(
    empty_command_list,
    "Should exit with an error about the command list being empty.",
);

make_snapshot_test!(
    program_not_found,
    "Should exit with an error about the program not being found.",
);

make_snapshot_test!(
    environment,
    "Should exit successfully after making use of a configured environment \
     variable.",
);

make_snapshot_test!(
    invalid_name_empty,
    "Should exit with an error about empty process names not being allowed",
);

make_snapshot_test!(
    invalid_name_start,
    "Should exit with an error about the first character in a process name \
     not being allowed",
);

make_snapshot_test!(
    invalid_name_continue,
    "Should exit with an error about a non-first character in a process name \
     not being allowed",
);

make_snapshot_test!(
    global_unknown_fields,
    "Should exit with an error about unknown fields.",
);

make_snapshot_test!(
    process_unknown_fields,
    "Should exit with an error about unknown fields.",
);

#[test]
fn alternate_file() -> TestResult {
    let td = tempdir()?;

    fs::copy(
        "tests/integrations/fixtures/minimal.toml",
        path!(td / "engage.toml"),
    )?;
    fs::copy(
        "tests/integrations/fixtures/one_process.toml",
        path!(td / "other.toml"),
    )?;

    let output = Command::new(cargo_bin!("engage"))
        .current_dir(&td)
        .args(["-f", "other.toml"])
        .output()?;

    let stdout = String::from_utf8(strip(output.stdout))?;
    let stderr = String::from_utf8(strip(output.stderr))?;
    let status_code = output.status.code();

    insta::with_settings!({
        description => "Should successfully run the process in `other.toml`.",
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
        .replace("process-a", "[redacted process name]")
        .replace("process-b", "[redacted process name]")
        .replace("process-c", "[redacted process name]")
        .replace("exit status: 1", "[redacted exit status]")
        .replace("exit status: 2", "[redacted exit status]")
        .replace("exit status: 3", "[redacted exit status]");
    let status_code = output.status.code();

    insta::with_settings!({
        description => "Should display multiple process failures in a \
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

// Tests that processes are spawned as soon as their dependencies have exited
// successfully, rather than being blocked on other processes they don't have an
// explicit (direct or transitive) dependency on.
#[tokio::test]
async fn a_then_b_and_c() -> TestResult {
    let mut child = tokio::process::Command::new(cargo_bin!("engage"))
        .args(["--file", "tests/integrations/fixtures/a_then_b_and_c.toml"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout = child.stdout.take().expect("stdout should be set");
    let mut haystack = Vec::new();

    let found = timeout(
        Duration::from_secs(1),
        is_needle_in_haystack(b"pass", &mut haystack, &mut stdout),
    )
    .await
    .expect("timer should not elapse")
    .expect("should be able to read child stdout");
    assert!(
        found,
        "process `c` should run despite process `a` sleeping forever"
    );

    kill(child.pid().expect("child should still be running"), Signal::SIGINT)
        .expect("should be able to kill child");

    child.wait().await?;

    Ok(())
}

// Tests that SIGINT handling can end processes early and not start processes
// further along the dependency tree.
#[tokio::test]
async fn sigint() -> TestResult {
    let mut child = tokio::process::Command::new(cargo_bin!("engage"))
        .args(["--file", "tests/integrations/fixtures/sigint.toml"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout = child.stdout.take().expect("stdout should be set");
    let mut haystack = Vec::new();

    let found = timeout(
        Duration::from_secs(1),
        is_needle_in_haystack(b"sleeping", &mut haystack, &mut stdout),
    )
    .await
    .expect("timer should not elapse")
    .expect("should be able to read child stdout");
    assert!(found, "child should have begun sleeping");

    kill(child.pid().expect("child should still be running"), Signal::SIGINT)
        .expect("should be able to kill child");

    let found = timeout(
        Duration::from_secs(1),
        is_needle_in_haystack(b"sigint", &mut haystack, &mut stdout),
    )
    .await
    .expect("timer should not elapse")
    .expect("should be able to read child stdout");
    assert!(found, "child should have been killed in its sleep");

    let found = timeout(
        Duration::from_secs(1),
        is_needle_in_haystack(b"never", &mut haystack, &mut stdout),
    )
    .await
    .expect("timer should not elapse")
    .expect("should be able to read child stdout");
    assert!(
        !found,
        "processes further along the dependency tree should not be started",
    );

    child.wait().await?;

    Ok(())
}
