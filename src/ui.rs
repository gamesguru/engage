//! Things to do with the "user interface" of the command line tool

use std::{fmt, io::stdout, process::Stdio, sync::Arc};

use crossterm::{
    execute,
    style::{Attribute, Print, SetAttribute, Stylize},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
};

use crate::{error, file, ui};

/// The separator that appears between the task's name and group and its output
pub static OUTPUT_SEPARATOR: &str = "│";

/// The separator between the task group and name
pub static TASK_GROUP_NAME_SEPARATOR: &str = "::";

/// Distinguish between `stdout` and `stderr`
enum StdKind {
    /// `stdout`
    Out,

    /// `stderr`
    Err,
}

/// Formats an error message to be printed on the command line
///
/// The returned string includes a trailing newline.
#[must_use]
pub fn format_error<D>(error: D) -> String
where
    D: fmt::Display,
{
    let mut buf = Vec::new();

    execute!(
        buf,
        Print("error".red().bold()),
        Print(':'.bold()),
        Print(' '),
        Print(error),
        Print('\n'),
    )
    .expect("should be able to write to in-memory buffer");

    String::from_utf8(buf).expect("should be a valid UTF-8 string")
}

/// Get a unique combination of group and task names
pub fn names_to_prefix<S1, S2>(group: S1, task: S2) -> String
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    format!("{}{}{}", group.as_ref(), TASK_GROUP_NAME_SEPARATOR, task.as_ref())
}

/// Returns the length of the longest prefix
#[must_use]
pub fn longest_prefix(file: &file::File) -> usize {
    let mut longest = 0;
    for task in &file.tasks {
        let length = names_to_prefix(&task.group, &task.name).len();

        if length > longest {
            longest = length;
        }
    }

    longest
}

/// Repeats the output from the `reader` prefixed with the task info
async fn repeat_prefixed<R>(
    longest_prefix: usize,
    kind: StdKind,
    reader: R,
    task: Arc<file::Task>,
) -> Result<(), error::Task>
where
    R: AsyncRead + Unpin,
{
    let buf_reader = BufReader::new(reader);
    let mut lines = buf_reader.lines();

    loop {
        let line = lines.next_line().await.map_err(error::Task::Read)?;

        if let Some(line) = line {
            let mut stdout = stdout().lock();

            execute!(
                stdout,
                SetAttribute(Attribute::Reset),
                Print(format!(
                    "{:>width$} ",
                    ui::names_to_prefix(&task.group, &task.name),
                    width = longest_prefix,
                )),
                Print(match kind {
                    StdKind::Out => OUTPUT_SEPARATOR.green(),
                    StdKind::Err => OUTPUT_SEPARATOR.red(),
                }),
                Print(format!(" {}", line)),
                Print('\n'),
            )
            .expect("failed to write output");
        } else {
            break;
        }
    }
    Ok(())
}

/// Try to run a task
///
/// # Errors
///
/// This can fail for a number of reasons, see [`error::Task`][error::Task]
/// for details.
pub async fn run_task(
    file: &file::File,
    longest_prefix: usize,
    task: Arc<file::Task>,
) -> Result<(), error::Task> {
    let mut child = Command::new(
        file.interpreter
            .get(0)
            .expect("interpreter should be set to a non-empty list"),
    )
    .args(&file.interpreter)
    .arg(&task.script)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(error::Task::Spawn)?;

    let mut handles = [None, None];

    if let Some(stdout) = child.stdout.take() {
        handles[0] = Some(tokio::spawn(repeat_prefixed(
            longest_prefix,
            StdKind::Out,
            stdout,
            task.clone(),
        )));
    }

    if let Some(stderr) = child.stderr.take() {
        handles[1] = Some(tokio::spawn(repeat_prefixed(
            longest_prefix,
            StdKind::Err,
            stderr,
            task.clone(),
        )));
    }

    for handle in handles.iter_mut().filter_map(Option::take) {
        handle.await.expect("failed to join task")?;
    }

    let status = child.wait().await.map_err(error::Task::Wait)?;

    if !status.success()
        && !status.code().map_or(false, |code| task.ignored.contains(&code))
    {
        return Err(error::Task::ExitStatus(status));
    }

    Ok(())
}
