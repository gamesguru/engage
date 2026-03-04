//! Implementation of running processes in a graph.

use std::{
    collections::HashMap, ops::ControlFlow, path::Path, process::Stdio,
    sync::Arc,
};

use futures_concurrency::future::FutureExt as _;
use futures_util::FutureExt as _;
use nix::sys::signal::{Signal, kill};
use petgraph::Direction;
use tokio::{
    io::{AsyncBufReadExt as _, AsyncRead, BufReader},
    process::Command,
    sync::{Notify, mpsc},
    task::JoinHandle,
};
use tokio_stream::{StreamExt as _, wrappers::ReceiverStream};
use tokio_util::sync::CancellationToken;
use tracing::Instrument as _;
use util::{ChildExt as _, DropGuard, SyncUnsafeCell};

use crate::{
    config::{Process, ReadyWhen},
    error, graph,
    name::Named,
    observability::prelude as o,
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
    ready_ct: CancellationToken,
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

    if process.value.ready_when == ReadyWhen::Spawned {
        ready_ct.cancel();
    }

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
        if process.value.ready_when == ReadyWhen::Exited {
            ready_ct.cancel();
        }
        Ok(())
    } else if cancelled {
        o::Span::current().record("otel.status_description", "cancelled");
        Err(E::CancelledWithError(status))
    } else {
        Err(E::ExitedWithError(status))
    }
}

/// Process state.
struct State {
    /// Handle to the process' task, if any.
    handle: Option<JoinHandle<ControlFlow<()>>>,

    /// Channel for sending the process signals.
    signal_tx: mpsc::Sender<()>,

    /// Cancellation token representing when the process becomes ready.
    ready_ct: CancellationToken,
}

impl State {
    /// Create a new [`State`] and associated signal receiver.
    fn new() -> (Self, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel(1);
        (
            Self {
                handle: None,
                signal_tx: tx,
                ready_ct: CancellationToken::new(),
            },
            rx,
        )
    }
}

/// Run a graph built from an Engage file.
#[o::instrument(skip_all, fields(otel.status_code = o::Empty))]
pub(crate) async fn run_graph(
    cancelled: Arc<Notify>,
    graph: Arc<graph::ProcessGraph>,
    root_dir: Arc<Path>,
) -> Result<(), error::RunGraph> {
    use error::RunGraph as E;

    let mut otel_status_code = DropGuard::new(o::OtelStatusCode::Error, |x| {
        x.record(&o::Span::current());
    });

    let span = o::Span::current();

    let (error_tx, error_rx) = mpsc::channel(1);
    let errors =
        tokio::spawn(ReceiverStream::new(error_rx).collect::<Vec<_>>());

    let states = Arc::new(
        graph
            .node_indices()
            .map(|ix| (ix, SyncUnsafeCell::<Option<State>>::default()))
            .collect::<HashMap<_, _>>(),
    );

    graph::edge_order_par_visit(
        cancelled.clone().notified(),
        &graph,
        Direction::Incoming,
        {
            let span = span.clone();
            let cancelled = cancelled.clone();
            let graph = graph.clone();
            let states = states.clone();
            move |ix| {
                let _enter = span.enter();

                let (state, signal_rx) = {
                    // SAFETY: Accesses to unique indices are serialized.
                    let state_opt = unsafe { &mut *states[&ix].get() };
                    let (state, signal_rx) = State::new();
                    (state_opt.insert(state), signal_rx)
                };

                start_process(
                    cancelled.clone().notified_owned(),
                    graph[ix].clone(),
                    root_dir.clone(),
                    state,
                    signal_rx,
                    error_tx.clone(),
                )
                .in_current_span()
            }
        },
    )
    .await;

    // Wait for the user to request cancellation unless:
    //
    // * All leaf processes are `ReadyWhen::Exited`.
    // * There are no processes.
    if !(graph
        .externals(Direction::Outgoing)
        .all(|ix| graph[ix].value.ready_when == ReadyWhen::Exited)
        || graph.node_count() == 0)
    {
        o::debug!("waiting for cancellation request");
        cancelled.notified().await;
    }

    graph::edge_order_par_visit(
        std::future::pending(),
        &graph,
        Direction::Outgoing,
        {
            let span = span.clone();
            let graph = graph.clone();
            let states = states.clone();
            move |ix| {
                let _enter = span.enter();

                // SAFETY: Accesses to unique indices are serialized.
                let state = unsafe { &mut *states[&ix].get() };

                finish_process(graph[ix].clone(), state).in_current_span()
            }
        },
    )
    .await;

    let errors = errors.await.expect("should be able to join errors task");

    if errors.is_empty() {
        *otel_status_code = o::OtelStatusCode::Ok;
        Ok(())
    } else {
        Err(E(errors))
    }
}

/// Start a process, returning when it becomes ready or if it fails before doing
/// so.
async fn start_process<C>(
    cancelled: C,
    process: Arc<Named<Process>>,
    root_dir: Arc<Path>,
    state: &mut State,
    signal_rx: mpsc::Receiver<()>,
    error_tx: mpsc::Sender<error::ProcessContext>,
) -> ControlFlow<()>
where
    C: Future<Output = ()>,
{
    let handle = tokio::spawn(
        {
            let process = process.clone();
            let ready_ct = state.ready_ct.clone();
            async move {
                let res = run_process(
                    async move {
                        let mut signal_rx = signal_rx;
                        signal_rx.recv().await;
                    }
                    .in_current_span(),
                    process.clone(),
                    root_dir,
                    ready_ct,
                )
                .await;

                if let Err(e) = res {
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
        }
        .in_current_span(),
    );

    if process.value.ready_when == ReadyWhen::Exited {
        // Wait for the process to complete.
        let wait = async { handle.await.expect("should be able to join task") };

        // Cancel the process if requested.
        let cancel = async {
            cancelled.await;

            state
                .signal_tx
                .send(())
                .await
                .expect("should be able to send signal");

            ControlFlow::Break(())
        };

        wait.race(cancel).await
    } else {
        state.handle = Some(handle);
        state.ready_ct.cancelled().await;
        ControlFlow::Continue(())
    }
}

/// Finish a process, returning when the process has finished.
///
/// If the process is still running, this function will signal it to stop first.
#[o::instrument(
    skip_all,
    fields(process.name = AsRef::<str>::as_ref(&process.name)),
)]
async fn finish_process(
    process: Arc<Named<Process>>,
    state: &mut Option<State>,
) -> ControlFlow<()> {
    let Some(state) = state else {
        o::debug!("never started, skipping");
        return ControlFlow::Continue(());
    };

    // If sending fails, it's because the process has completed already.
    let _: Result<(), mpsc::error::SendError<()>> =
        state.signal_tx.send(()).await;

    if let Some(handle) = state.handle.take() {
        // Ignore return value to continue finishing other processes.
        let _: ControlFlow<()> =
            handle.await.expect("should be able to join task");
    }

    ControlFlow::Continue(())
}
