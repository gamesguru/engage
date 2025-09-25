//! Things to do with the "user interface" of the command line tool.

use std::{
    fmt::Write as _, num::NonZeroUsize, ops::ControlFlow, process::Stdio,
    sync::Arc,
};

use crossterm::style::{Attribute, SetAttribute, Stylize as _};
use petgraph::graph::{DiGraph, IndexType};
use tokio::{
    io::{AsyncBufReadExt as _, AsyncRead, BufReader},
    process::Command,
    sync::{Semaphore, mpsc},
};

use crate::{
    config::{Config, Task},
    error, graph,
    name::Named,
};

mod unicode {
    #![allow(missing_docs)]
    #![allow(clippy::missing_docs_in_private_items)]

    //! Unicode characters used in the UI.

    pub(crate) const BLACK_LEFT_POINTING: char = '\u{25C0}';
    pub(crate) const BLACK_RIGHT_POINTING: char = '\u{25B6}';
    pub(crate) const LIGHT_ARC_DOWN_AND_RIGHT: char = '\u{256D}';
    pub(crate) const LIGHT_ARC_UP_AND_RIGHT: char = '\u{2570}';
    pub(crate) const LIGHT_DOWN_AND_HORIZONTAL: char = '\u{252C}';
    pub(crate) const LIGHT_HORIZONTAL: char = '\u{2500}';
    pub(crate) const LIGHT_UP_AND_HORIZONTAL: char = '\u{2534}';
    pub(crate) const LIGHT_VERTICAL: char = '\u{2502}';
}

/// Distinguish between `stdout` and `stderr`.
enum StdKind {
    /// `stdout`.
    Out,

    /// `stderr`.
    Err,
}

/// The sequence of characters to print.
#[derive(Clone, Copy)]
pub(crate) enum Sequence {
    /// The start sequence.
    Start,

    /// The end sequence.
    End,
}

/// Write the start sequence to a string for printing.
pub(crate) fn fmt_sequence(
    sequence: Sequence,
    longest_prefix: usize,
) -> String {
    let mut buf = String::new();

    let d = unicode::LIGHT_HORIZONTAL;
    let t = match sequence {
        Sequence::Start => unicode::LIGHT_DOWN_AND_HORIZONTAL,
        Sequence::End => unicode::LIGHT_UP_AND_HORIZONTAL,
    };
    let a = match sequence {
        Sequence::Start => unicode::LIGHT_ARC_DOWN_AND_RIGHT,
        Sequence::End => unicode::LIGHT_ARC_UP_AND_RIGHT,
    };
    let p = match sequence {
        Sequence::Start => unicode::BLACK_LEFT_POINTING,
        Sequence::End => unicode::BLACK_RIGHT_POINTING,
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

/// Returns the length of the longest task name.
#[must_use]
fn longest_name<I, S>(names: I) -> usize
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut longest = 0;
    for name in names {
        let length = name.as_ref().len();

        if length > longest {
            longest = length;
        }
    }

    longest
}

/// Repeats the output from the `reader` prefixed with the task info.
async fn repeat_prefixed<R>(
    longest_prefix: usize,
    kind: StdKind,
    reader: R,
    task: Arc<Named<Task>>,
) -> Result<(), error::Task>
where
    R: AsyncRead + Unpin,
{
    use error::Task as E;

    let buf_reader = BufReader::new(reader);
    let mut lines = buf_reader.lines();

    loop {
        let line = lines.next_line().await.map_err(|e| E::Read(e.into()))?;

        if let Some(line) = line {
            let kind = match kind {
                StdKind::Out => 'O',
                StdKind::Err => 'E',
            };

            println!(
                "{}{:>longest_prefix$} {sep}{kind}{sep} {line}",
                SetAttribute(Attribute::Reset),
                &task.name,
                sep = unicode::LIGHT_VERTICAL.blue(),
            );
        } else {
            break;
        }
    }
    Ok(())
}

/// Try to run a task.
///
/// # Errors
///
/// This can fail for a number of reasons, see [`error::Task`] for details.
async fn run_task(
    longest_prefix: usize,
    task: Arc<Named<Task>>,
) -> Result<(), error::Task> {
    use error::Task as E;

    let command = task
        .value
        .command
        .first()
        .expect("command should have at least 1 element");

    let mut child = Command::new(command)
        .args(&task.value.command[1..])
        .envs(&task.value.environment)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| E::Spawn(e.into(), command.clone()))?;

    let stdout = tokio::spawn(repeat_prefixed(
        longest_prefix,
        StdKind::Out,
        child.stdout.take().expect("should be able to take child stdout"),
        task.clone(),
    ));

    let stderr = tokio::spawn(repeat_prefixed(
        longest_prefix,
        StdKind::Err,
        child.stderr.take().expect("should be able to take child stderr"),
        task.clone(),
    ));

    stdout.await.expect("should be able to join stdout")?;
    stderr.await.expect("should be able to join stderr")?;

    let status = child.wait().await.map_err(|e| E::Wait(e.into()))?;

    if !status.success() {
        return Err(E::ExitStatus(status));
    }

    Ok(())
}

/// Run all tasks in the given graph based on the Engage file.
pub(crate) async fn run_graph<E, Ix>(
    graph: DiGraph<graph::Node, E, Ix>,
    config: Config,
    max_parallelism: Option<NonZeroUsize>,
) -> Result<(), error::RunGraph>
where
    E: Send + Sync + 'static,
    Ix: IndexType + Send + Sync,
{
    use error::RunGraph as E;

    graph::ensure_acyclic(&graph).map_err(E::Cyclic)?;
    let longest_name = longest_name(config.tasks.keys());
    let semaphore = max_parallelism.map(|x| Arc::new(Semaphore::new(x.get())));

    println!(
        "{} {}",
        fmt_sequence(Sequence::Start, longest_name).blue(),
        "starting".blue().bold(),
    );

    let (error_tx, mut error_rx) = mpsc::channel(16);
    let error_collector = tokio::spawn(async move {
        let mut errors = Vec::new();

        while let Some(next) = error_rx.recv().await {
            errors.push(next);
        }

        errors
    });

    graph::execute(&graph, move |task| {
        let semaphore = semaphore.clone();
        let error_tx = error_tx.clone();
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

            let task = Arc::new(task);
            if let Err(e) = run_task(longest_name, task.clone()).await {
                error_tx
                    .send((task, e))
                    .await
                    .expect("channel should still be open");

                return ControlFlow::Break(());
            }

            drop(permit);

            ControlFlow::Continue(())
        }
    })
    .await;

    let errors = error_collector
        .await
        .expect("should be able to join task")
        .into_iter()
        .map(|(task, error)| error::TaskContext {
            name: task.name.clone(),
            child: error,
        })
        .collect::<Vec<_>>();

    if errors.is_empty() {
        println!(
            "{}{} {}",
            SetAttribute(Attribute::Reset),
            fmt_sequence(Sequence::End, longest_name).blue(),
            "success".bold().green(),
        );

        Ok(())
    } else {
        println!(
            "{}{} {}",
            SetAttribute(Attribute::Reset),
            fmt_sequence(Sequence::End, longest_name).blue(),
            "failure".bold().red(),
        );

        Err(E::Task(errors))
    }
}
