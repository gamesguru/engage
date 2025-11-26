//! Implementation of running tasks in a graph.

use std::{num::NonZeroUsize, ops::ControlFlow, process::Stdio, sync::Arc};

use petgraph::graph::{DiGraph, IndexType};
use tokio::{
    io::{AsyncBufReadExt as _, AsyncRead, BufReader},
    process::Command,
    sync::{Semaphore, mpsc},
};
use tracing::Instrument as _;

use crate::{
    config::Task, error, graph, name::Named, observability::prelude as o,
    util::DropGuard,
};

/// Output kind.
pub(crate) enum OutputKind {
    /// `stdout`.
    Stdout,

    /// `stderr`.
    Stderr,
}

impl OutputKind {
    /// Convert a string into [`Self`].
    ///
    /// # Panics
    ///
    /// Panics if the input string is invalid.
    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "stdout" => Self::Stdout,
            "stderr" => Self::Stderr,
            _ => panic!("invalid input"),
        }
    }

    /// Get a unique character for the current value.
    pub(crate) fn to_char(&self) -> char {
        match self {
            OutputKind::Stdout => 'O',
            OutputKind::Stderr => 'E',
        }
    }
}

impl AsRef<str> for OutputKind {
    fn as_ref(&self) -> &str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// Repeats the output from the `reader` prefixed with the task info.
async fn repeat_prefixed<R>(
    kind: OutputKind,
    reader: R,
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
            o::info!(kind = AsRef::<str>::as_ref(&kind), data = line, "output");
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
#[o::instrument(
    skip_all,
    fields(
        // NOTE: Can't just call it `name` because that conflicts with the span
        // name in tracing-subscriber's JSON output format.
        task.name = AsRef::<str>::as_ref(&task.name),
        otel.status_code = o::Empty,
    ),
)]
async fn run_task(task: Arc<Named<Task>>) -> Result<(), error::Task> {
    use error::Task as E;

    let mut otel_status_code = DropGuard::new(o::OtelStatusCode::Error, |x| {
        x.record(&o::Span::current());
    });

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

    let stdout = tokio::spawn(
        repeat_prefixed(
            OutputKind::Stdout,
            child.stdout.take().expect("should be able to take child stdout"),
        )
        .in_current_span(),
    );

    let stderr = tokio::spawn(
        repeat_prefixed(
            OutputKind::Stderr,
            child.stderr.take().expect("should be able to take child stderr"),
        )
        .in_current_span(),
    );

    stdout.await.expect("should be able to join stdout")?;
    stderr.await.expect("should be able to join stderr")?;

    let status = child.wait().await.map_err(|e| E::Wait(e.into()))?;

    if status.success() {
        *otel_status_code = o::OtelStatusCode::Ok;
        Ok(())
    } else {
        Err(E::ExitStatus(status))
    }
}

/// Run all tasks in the given graph based on the Engage file.
#[o::instrument(skip_all, fields(otel.status_code = o::Empty))]
pub(crate) async fn run_graph<E, Ix>(
    graph: Arc<DiGraph<Arc<Named<Task>>, E, Ix>>,
    max_parallelism: Option<NonZeroUsize>,
) -> Result<(), error::RunGraph>
where
    E: Send + Sync + 'static,
    Ix: IndexType + Send + Sync,
{
    use error::RunGraph as E;

    let mut otel_status_code = DropGuard::new(o::OtelStatusCode::Error, |x| {
        x.record(&o::Span::current());
    });

    let span = o::Span::current();

    let semaphore = max_parallelism.map(|x| Arc::new(Semaphore::new(x.get())));

    let (error_tx, mut error_rx) = mpsc::channel(16);
    let error_collector = tokio::spawn(async move {
        let mut errors = Vec::new();

        while let Some(next) = error_rx.recv().await {
            errors.push(next);
        }

        errors
    });

    graph::edge_order_par_visit(&graph, {
        let graph = graph.clone();
        let span = span.clone();
        move |ix| {
            let _enter = span.enter();
            let task = graph[ix].clone();
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

                if let Err(e) = run_task(task.clone()).await {
                    error_tx
                        .send((task, e))
                        .await
                        .expect("channel should still be open");

                    return ControlFlow::Break(());
                }

                drop(permit);

                ControlFlow::Continue(())
            }
            .in_current_span()
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
        *otel_status_code = o::OtelStatusCode::Ok;
        Ok(())
    } else {
        Err(E(errors))
    }
}
