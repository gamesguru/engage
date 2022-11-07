//! Things to do with the "user interface" of the command line tool

use std::{cmp, ops::ControlFlow, process::Stdio, sync::Arc};

use crossterm::style::{Attribute, SetAttribute, Stylize};
use petgraph::graph::{DiGraph, IndexType};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
};

use crate::{error, file, graph};

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
fn longest_prefix(file: &file::File) -> usize {
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
            let sep = match kind {
                StdKind::Out => OUTPUT_SEPARATOR.green(),
                StdKind::Err => OUTPUT_SEPARATOR.red(),
            };

            println!(
                "{}{:>longest_prefix$} {sep} {line}",
                SetAttribute(Attribute::Reset),
                names_to_prefix(&task.group, &task.name),
            );
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
async fn run_task(
    file: &file::File,
    longest_prefix: usize,
    task: Arc<file::Task>,
) -> Result<(), error::Task> {
    let command = file
        .interpreter
        .get(0)
        .expect("file should be validated before running any tasks");

    let mut child = Command::new(command)
        .args(&file.interpreter[1..])
        .arg(&task.script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| error::Task::Spawn(e, command.clone()))?;

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

/// Run all groups and tasks in the given graph based on the Engage file
pub async fn run_graph<E, Ix>(
    graph: DiGraph<graph::Node, E, Ix>,
    file: file::File,
    internal_prefix: String,
) -> Result<(), error::RunGraph>
where
    E: Send + Sync + 'static,
    Ix: IndexType + Send + Sync,
{
    graph::ensure_acyclic(&graph)?;
    let longest_prefix = cmp::max(internal_prefix.len(), longest_prefix(&file));
    let file = Arc::new(file);

    graph::execute(Arc::new(graph), move |node| {
        let file = file.clone();
        async move {
            if let graph::Node::Task(task) = node {
                let task = Arc::new(task);
                if let Err(e) =
                    run_task(&file, longest_prefix, task.clone()).await
                {
                    return ControlFlow::Break((task, e));
                }
            }

            ControlFlow::Continue(())
        }
    })
    .await
    .map_or_else(
        || {
            println!(
                "{}{:>longest_prefix$} {} {}",
                SetAttribute(Attribute::Reset),
                internal_prefix,
                // Pretend this came from `stdout`
                OUTPUT_SEPARATOR.green(),
                "success".bold().green(),
            );

            Ok(())
        },
        |(task, error)| {
            Err(error::RunGraph::Task {
                longest_prefix,
                source: error,
                task,
            })
        },
    )
}
