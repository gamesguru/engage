//! Implementation of running processes in a graph.

use std::{ops::ControlFlow, path::Path, process::Stdio, sync::Arc};

use futures_concurrency::future::FutureExt as _;
use futures_util::FutureExt as _;
use nix::sys::signal::{Signal, kill};
use petgraph::graph::{DiGraph, IndexType};
use tokio::{
    io::{AsyncBufReadExt as _, AsyncRead, BufReader},
    process::Command,
    sync::{Notify, mpsc},
};
use tokio_stream::{StreamExt as _, wrappers::ReceiverStream};
use tracing::Instrument as _;
use util::{ChildExt as _, DropGuard};

use crate::{
    config::Process, error, graph, name::Named, observability::prelude as o,
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

/// Repeats the output from the `reader` prefixed with the process info.
async fn repeat_prefixed<R>(
    kind: OutputKind,
    reader: R,
) -> Result<(), error::Process>
where
    R: AsyncRead + Unpin,
{
    use error::Process as E;

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

/// Try to run a process.
///
/// The process will be signalled to exit if `cancelled` completes. `root_dir`
/// should be an absolute path to the parent directory of the Engage file in
/// use.
///
/// # Errors
///
/// This can fail for a number of reasons, see [`error::Process`] for details.
#[o::instrument(
    skip_all,
    fields(
        // NOTE: Can't just call it `name` because that conflicts with the span
        // name in tracing-subscriber's JSON output format.
        process.name = AsRef::<str>::as_ref(&process.name),
        otel.status_code = o::Empty,
        otel.status_description = o::Empty,
    ),
)]
async fn run_process<C>(
    cancelled: C,
    process: Arc<Named<Process>>,
    root_dir: Arc<Path>,
) -> Result<(), error::Process>
where
    C: Future<Output = ()>,
{
    use error::Process as E;

    let mut otel_status_code = DropGuard::new(o::OtelStatusCode::Error, |x| {
        x.record(&o::Span::current());
    });

    let command = process
        .value
        .command
        .first()
        .expect("command should have at least 1 element");

    let mut child = Command::new(command)
        .args(&process.value.command[1..])
        .envs(&process.value.environment)
        .current_dir(&*root_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
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

    let wait = child.wait().map(|x| x.map(Some).map_err(|e| E::Wait(e.into())));
    let cancelled = cancelled.map(|()| Ok(None));
    let status = wait.race(cancelled).await?;

    let (status, cancelled) = if let Some(status) = status {
        (status, false)
    } else {
        if let Some(pid) = child.pid() {
            kill(pid, Signal::SIGINT).map_err(|e| E::Signal(e.into()))?;
        }

        (child.wait().await.map_err(|e| E::Wait(e.into()))?, true)
    };

    stdout.await.expect("should be able to join stdout")?;
    stderr.await.expect("should be able to join stderr")?;

    if status.success() {
        *otel_status_code = o::OtelStatusCode::Ok;
        Ok(())
    } else if cancelled {
        o::Span::current().record("otel.status_description", "cancelled");
        Err(E::CancelledWithError(status))
    } else {
        Err(E::ExitedWithError(status))
    }
}

/// Run a graph built from an Engage file.
#[o::instrument(skip_all, fields(otel.status_code = o::Empty))]
pub(crate) async fn run_graph<E, Ix>(
    cancelled: Arc<Notify>,
    graph: Arc<DiGraph<Arc<Named<Process>>, E, Ix>>,
    root_dir: Arc<Path>,
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

    let (error_tx, error_rx) = mpsc::channel(1);
    let errors =
        tokio::spawn(ReceiverStream::new(error_rx).collect::<Vec<_>>());

    graph::edge_order_par_visit(cancelled.clone().notified(), &graph, {
        let graph = graph.clone();
        let span = span.clone();
        move |ix| {
            let _enter = span.enter();
            let process = graph[ix].clone();
            let error_tx = error_tx.clone();
            let cancelled = cancelled.clone();
            let root_dir = root_dir.clone();
            async move {
                if let Err(e) =
                    run_process(cancelled.notified(), process.clone(), root_dir)
                        .await
                {
                    error_tx
                        .send(error::ProcessContext {
                            name: process.name.clone(),
                            child: e,
                        })
                        .await
                        .expect("channel should still be open");

                    return ControlFlow::Break(());
                }

                ControlFlow::Continue(())
            }
            .in_current_span()
        }
    })
    .await;

    let errors = errors.await.expect("should be able to join errors task");

    if errors.is_empty() {
        *otel_status_code = o::OtelStatusCode::Ok;
        Ok(())
    } else {
        Err(E(errors))
    }
}
