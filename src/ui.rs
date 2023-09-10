//! Things to do with the "user interface" of the command line tool

use std::{
    fmt::Write, iter, num::NonZeroUsize, ops::ControlFlow, process::Stdio,
    sync::Arc,
};

use crossterm::style::{Attribute, SetAttribute, Stylize};
use petgraph::graph::{DiGraph, IndexType};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::{ChildStderr, ChildStdout, Command},
    sync::Semaphore,
};

use crate::{error, file, graph};

/// The separator between the task group and name
pub static TASK_GROUP_NAME_SEPARATOR: &str = "::";

mod unicode {
    #![allow(missing_docs)]
    #![allow(clippy::missing_docs_in_private_items)]

    //! Unicode characters used in the UI

    pub const BLACK_LEFT_POINTING: char = '\u{25C0}';
    pub const BLACK_RIGHT_POINTING: char = '\u{25B6}';
    pub const LIGHT_ARC_DOWN_AND_RIGHT: char = '\u{256D}';
    pub const LIGHT_ARC_UP_AND_RIGHT: char = '\u{2570}';
    pub const LIGHT_DOWN_AND_HORIZONTAL: char = '\u{252C}';
    pub const LIGHT_HORIZONTAL: char = '\u{2500}';
    pub const LIGHT_UP_AND_HORIZONTAL: char = '\u{2534}';
    pub const LIGHT_VERTICAL: char = '\u{2502}';
    pub const LIGHT_VERTICAL_AND_RIGHT: char = '\u{251C}';
    pub const LIGHT_VERTICAL_AND_HORIZONTAL: char = '\u{253C}';
}

/// Output of a child process
enum ChildStdo {
    /// `stdout`
    Out(ChildStdout),

    /// `stderr`
    Err(ChildStderr),
}

/// The sequence of characters to print
#[derive(Clone, Copy)]
pub enum Sequence {
    /// The start sequence
    Start,

    /// A sequence to be printed between the start and end
    Middle,

    /// The end sequence
    End,
}

/// Write the start sequence to a string for printing
pub fn fmt_sequence(sequence: Sequence, longest_prefix: usize) -> String {
    let mut buf = String::new();

    let d = unicode::LIGHT_HORIZONTAL;
    let t = match sequence {
        Sequence::Start => unicode::LIGHT_DOWN_AND_HORIZONTAL,
        Sequence::Middle => unicode::LIGHT_VERTICAL_AND_HORIZONTAL,
        Sequence::End => unicode::LIGHT_UP_AND_HORIZONTAL,
    };
    let a = match sequence {
        Sequence::Start => unicode::LIGHT_ARC_DOWN_AND_RIGHT,
        Sequence::Middle => unicode::LIGHT_VERTICAL_AND_RIGHT,
        Sequence::End => unicode::LIGHT_ARC_UP_AND_RIGHT,
    };
    let p = match sequence {
        Sequence::Start => unicode::BLACK_LEFT_POINTING,
        Sequence::Middle | Sequence::End => unicode::BLACK_RIGHT_POINTING,
    };

    let mut try_f = || {
        for _ in 0..longest_prefix {
            write!(buf, " ")?;
        }

        write!(buf, " {a}{d}{t}{p}")
    };

    try_f().expect("write to in-memory buffer should succeed");

    buf
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
async fn repeat_prefixed(
    longest_prefix: usize,
    mut child_std_fd: ChildStdo,
    task: Arc<file::Task>,
) -> Result<(), error::Task> {
    let (reader, kind): (&mut (dyn AsyncRead + Unpin + Send), _) =
        match &mut child_std_fd {
            ChildStdo::Out(x) => (x, 'O'),
            ChildStdo::Err(x) => (x, 'E'),
        };

    let buf_reader = BufReader::new(reader);
    let mut lines = buf_reader.lines();

    loop {
        let line = lines.next_line().await.map_err(error::Task::Read)?;

        let Some(line) = line else {
            break;
        };

        println!(
            "{}{:>longest_prefix$} {sep}{kind}{sep} {line}",
            SetAttribute(Attribute::Reset),
            names_to_prefix(&task.group, &task.name),
            sep = unicode::LIGHT_VERTICAL.blue(),
        );
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

    let msg = "should be able to take child's std fd";

    let handles = iter::empty()
        .chain(iter::once(ChildStdo::Out(child.stdout.take().expect(msg))))
        .chain(iter::once(ChildStdo::Err(child.stderr.take().expect(msg))))
        .map(|child_std_fd| {
            tokio::spawn(repeat_prefixed(
                longest_prefix,
                child_std_fd,
                task.clone(),
            ))
        });

    for handle in handles {
        handle.await.expect("should be able to join task")?;
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
    max_parallelism: Option<NonZeroUsize>,
) -> Result<(), error::RunGraph>
where
    E: Send + Sync + 'static,
    Ix: IndexType + Send + Sync,
{
    graph::ensure_acyclic(&graph)?;
    let longest_prefix = longest_prefix(&file);
    let file = Arc::new(file);
    let semaphore = max_parallelism.map(|x| Arc::new(Semaphore::new(x.get())));

    println!(
        "{} {}",
        fmt_sequence(Sequence::Start, longest_prefix).blue(),
        "starting".blue().bold(),
    );

    let errors = graph::execute(Arc::new(graph), move |node| {
        let file = file.clone();
        let semaphore = semaphore.clone();
        async move {
            let permit = if let Some(semaphore) = semaphore {
                Some(
                    semaphore
                        .acquire_owned()
                        .await
                        .expect("semaphore shouldn't be closed"),
                )
            } else {
                None
            };

            if let graph::Node::Task(task) = node {
                let task = Arc::new(task);
                if let Err(e) =
                    run_task(&file, longest_prefix, task.clone()).await
                {
                    return ControlFlow::Break((task, e));
                }
            }

            drop(permit);

            ControlFlow::Continue(())
        }
    })
    .await;

    let errors_is_empty = errors.is_empty();
    let errors_len = errors.len();

    for (is_last, (task, error)) in
        errors.into_values().enumerate().map(|(i, x)| (i + 1 == errors_len, x))
    {
        let sequence = if is_last {
            Sequence::End
        } else {
            Sequence::Middle
        };

        println!(
            "{}{} {e}{c} {n} failed: {r}",
            SetAttribute(Attribute::Reset),
            fmt_sequence(sequence, longest_prefix).blue(),
            e = "failure".bold().red(),
            c = ':'.bold(),
            n = names_to_prefix(&task.group, &task.name),
            r = error::Chain(&error),
        );
    }
    if errors_is_empty {
        println!(
            "{}{} {}",
            SetAttribute(Attribute::Reset),
            fmt_sequence(Sequence::End, longest_prefix).blue(),
            "success".bold().green(),
        );

        Ok(())
    } else {
        Err(error::RunGraph::Task)
    }
}
