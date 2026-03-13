//! Error handling facilities.

use std::{
    borrow::Cow, collections::BTreeSet, fmt, io, path::PathBuf,
    process::ExitStatus, sync::Arc,
};

use derail::CoreCompat;
use derail_macros::Error;
use nix::errno::Errno;
use tracing_subscriber::filter::FromEnvError;

use crate::{
    config,
    graph::EdgeKind,
    name::{Name, Named},
};

pub(crate) mod report;

/// Error details.
pub(crate) struct Details {
    /// A recommendation for resolving the error.
    help: Option<Cow<'static, str>>,
}

impl Details {
    /// An empty collection of details.
    fn empty() -> Self {
        Details {
            help: None,
        }
    }

    /// Set a recommendation for resolving the error.
    fn help<S>(mut self, help: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        self.help = Some(help.into());
        self
    }
}

impl From<()> for Details {
    fn from((): ()) -> Self {
        Self::empty()
    }
}

/// There was an error running the program.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum Main {
    /// Aborted due to command line usage.
    #[derail(
        display("aborted due to command line usage"),
        details = Details::empty(),
    )]
    Cli,

    /// Failed to load the Engage file.
    #[derail(
        display("failed to load the Engage file"),
        details = Details::empty(),
    )]
    LoadConfig(#[derail(child)] LoadConfig),

    /// The Engage file contains invalid dependencies.
    #[derail(
        display("the Engage file contains invalid dependencies"),
        details = Details::empty()
            .help(
                "try using `engage dot` with the `-r`/`--relaxed` option to \
                 visualize the graph to assist in debugging and fixing the \
                 issues"
            ),
    )]
    BuildGraph(#[derail(children)] Vec<BuildGraph>),

    /// One or more processes were selected more than once.
    #[derail(
        display("one or more processes were selected more than once"),
        details = Details::empty(),
    )]
    ProcessesRepeated(#[derail(children)] BTreeSet<ProcessRepeated>),

    /// One or more of the selected processes were not found.
    #[derail(
        display("one or more of the selected processes were not found"),
        details = Details::empty(),
    )]
    ProcessesNotFound(#[derail(children)] BTreeSet<ProcessNotFound>),

    /// Failed to run the graph.
    RunGraph(#[derail(skip_self)] RunGraph),

    /// Failed to initialize observability.
    Observability(#[derail(skip_self)] Observability),
}

/// A process was selected more than once.
#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
#[derail(
    type Details = Details,
    display("the process `{_0}` was selected more than once"),
    details = Details::empty(),
)]
pub(crate) struct ProcessRepeated(pub(crate) Box<Name>);

/// Failed to initialize observability.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum Observability {
    /// Failed to parse the `RUST_LOG` environment variable.
    #[derail(
        display("failed to parse the RUST_LOG environment variable"),
        details = Details::empty(),
    )]
    FromEnvError(#[derail(skip_child, map_details)] CoreCompat<FromEnvError>),
}

/// Failed to find an Engage file.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum FileFind {
    /// Failed to read directory.
    #[derail(
        display("failed to read directory '{}'", _1.display()),
        details = Details::empty(),
    )]
    ReadDir(#[derail(child, map_details)] CoreCompat<io::Error>, PathBuf),

    /// Failed to get next directory entry.
    #[derail(
        display("failed to get next entry in directory '{}'", _1.display()),
        details = Details::empty(),
    )]
    NextEntry(#[derail(child, map_details)] CoreCompat<io::Error>, PathBuf),

    /// Failed to find an Engage file.
    #[derail(
        display(
            "{} not found in the current directory or its ancestors",
            config::DEFAULT_FILE_NAME,
        ),
        details = Details::empty()
            .help(
                "double check the current directory or specify the Engage file \
                 on the command line"
            ),
    )]
    NotFound,
}

/// Failed to load the Engage file.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum LoadConfig {
    /// Failed to determine the current directory.
    #[derail(
        display("failed to determine the current directory"),
        details = Details::empty(),
    )]
    CurrentDir(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// Failed to find an Engage file.
    #[derail(
        display("failed to find an Engage file"),
        details = Details::empty(),
    )]
    FileFind(#[derail(child, map_details)] FileFind),

    /// Failed to read the Engage file.
    #[derail(
        display("failed to read the Engage file"),
        details = Details::empty(),
    )]
    ReadFile(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// Failed to deserialize the Engage file.
    #[derail(
        display("failed to deserialize the Engage file"),
        details = Details::empty(),
    )]
    Deserialize(#[derail(child, map_details)] CoreCompat<toml::de::Error>),

    /// The Engage file contains errors.
    #[derail(
        display("the Engage file contains errors"),
        details = Details::empty(),
    )]
    File(#[derail(children)] Vec<File>),
}

/// A process failed to run.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum Process {
    /// Failed to spawn a process for the program.
    #[derail(
        display(
            "failed to spawn a process for the program \"{}\"",
            _1.escape_debug(),
        ),
        details = Details::empty(),
    )]
    Spawn(#[derail(child, map_details)] CoreCompat<io::Error>, String),

    /// Failed to read the process' output.
    #[derail(
        display("failed to read the process' output"),
        details = Details::empty(),
    )]
    Read(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// Failed to send a signal to the process.
    #[derail(
        display("failed to send a signal to the process"),
        details = Details::empty(),
    )]
    Signal(#[derail(child, map_details)] CoreCompat<Errno>),

    /// Failed to wait for the process to exit.
    #[derail(
        display("failed to wait for process to exit"),
        details = Details::empty(),
    )]
    Wait(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// The process exited unsuccessfully.
    #[derail(
        display("process exited unsuccessfully via {_0}"),
        details = Details::empty()
            .help("review this process' logs to determine the cause"),
    )]
    ExitedWithError(ExitStatus),

    /// The process exited due to cancellation unsuccessfully.
    #[derail(
        display("process exited due to cancellation unsuccessfully via {_0}"),
        details = Details::empty(),
    )]
    CancelledWithError(ExitStatus),
}

/// An error building the graph.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum BuildGraph {
    /// A process depends on a nonexistent process.
    #[derail(
        display(
            "`{process}` contains `{dependency}` in its \"{}\" list but the \
             latter does not exist",
            match edge_kind {
                EdgeKind::Before => "before",
                EdgeKind::After => "after",
            }
        ),
        details = Details::empty()
            .help(
                "either create the nonexistent process or remove it from the \
                 list"
            ),
    )]
    DependencyNotFound {
        /// The process in question.
        process: Box<Name>,

        /// The nonexistent process in the `before` or `after` list.
        dependency: Box<Name>,

        /// The edge kind.
        edge_kind: EdgeKind,
    },

    /// A process is part of a nonexistent process.
    #[derail(
        display(
            "`{process}` has \"part-of\" set to `{part_of}` but that process \
             does not exist"
        ),
        details = Details::empty()
            .help("either create the process or unset the \"part-of\" key"),
    )]
    PartOfNotFound {
        /// The process in question.
        process: Box<Name>,

        /// The nonexistent process it wants to be part of.
        part_of: Box<Name>,
    },

    /// A dependency is not part of the same process as this process and this
    /// process is part of any process.
    #[derail(
        display("{}", fmt::from_fn(|f| fmt_dependency_not_part_of(
            f,
            process,
            process_part_of,
            dependency,
            dependency_part_of.as_deref(),
            *edge_kind,
        ))),
        details = Details::empty()
            .help(format!(
                "either remove `{dependency}` from the list or make \
                 `{dependency}` part of `{process_part_of}`"
            )),
    )]
    DependencyNotPartOf {
        /// The process in question.
        process: Box<Name>,

        /// The process this process is part of.
        process_part_of: Box<Name>,

        /// The process in the `before` or `after` list that's not part of the
        /// same process as this process.
        dependency: Box<Name>,

        /// The process the dependency is part of, if any.
        dependency_part_of: Option<Box<Name>>,

        /// The edge kind.
        edge_kind: EdgeKind,
    },

    /// A process is not part of the same process as one of its dependencies and
    /// the dependency is part of any process.
    #[derail(
        display("{}", fmt::from_fn(|f| fmt_process_not_part_of(
            f,
            process,
            process_part_of.as_deref(),
            dependency,
            dependency_part_of,
            *edge_kind,
        ))),
        details = Details::empty()
            .help(format!(
                "either remove `{dependency}` from the list or make \
                 `{process}` part of `{dependency_part_of}`"
            )),
    )]
    ProcessNotPartOf {
        /// The process in question.
        process: Box<Name>,

        /// The process this process is part of, if any.
        process_part_of: Option<Box<Name>>,

        /// The process in the `before` or `after` list that's part of a
        /// different process than this process.
        dependency: Box<Name>,

        /// The process the dependency is part of.
        dependency_part_of: Box<Name>,

        /// The edge kind.
        edge_kind: EdgeKind,
    },

    /// A service wants to be part of a task.
    #[derail(
        display(
            "`{process}` is a service but it wants to be part of the task \
            `{part_of}`"),
        details = Details::empty()
            .help(format!(
                "either make `{process}` a task or make it not part of \
                 `{part_of}`"
            )),
    )]
    ServicePartOfTask {
        /// The process in question.
        process: Box<Name>,

        /// The process this process is part of and after.
        part_of: Box<Name>,
    },

    /// A dependency cycle.
    #[derail(
        display("{}", fmt::from_fn(|f| fmt_cycle(f, _0))),
        details = Details::empty(),
    )]
    DependencyCycle(Vec<Arc<Named<config::Process>>>),
}

/// Workaround for <https://gitlab.computer.surgery/charles/derail/-/issues/5>.
fn fmt_dependency_not_part_of(
    f: &mut fmt::Formatter<'_>,
    process: &Name,
    process_part_of: &Name,
    dependency: &Name,
    dependency_part_of: Option<&Name>,
    edge_kind: EdgeKind,
) -> fmt::Result {
    write!(
        f,
        "`{process}` is part of `{process_part_of}` and contains \
         `{dependency}` in its \"{}\" list, but `{dependency}` is ",
        match edge_kind {
            EdgeKind::Before => "before",
            EdgeKind::After => "after",
        },
    )?;

    match dependency_part_of {
        Some(x) => write!(f, "part of `{x}`"),
        None => write!(f, "not"),
    }
}

/// Workaround for <https://gitlab.computer.surgery/charles/derail/-/issues/5>.
fn fmt_process_not_part_of(
    f: &mut fmt::Formatter<'_>,
    process: &Name,
    process_part_of: Option<&Name>,
    dependency: &Name,
    dependency_part_of: &Name,
    edge_kind: EdgeKind,
) -> fmt::Result {
    write!(f, "`{process}` is ")?;

    match process_part_of {
        Some(x) => write!(f, "part of `{x}`")?,
        None => write!(f, "not part of another process")?,
    }

    write!(
        f,
        " and contains `{dependency}` in its \"{}\" list, but `{dependency}` \
         is part of `{dependency_part_of}`",
        match edge_kind {
            EdgeKind::Before => "before",
            EdgeKind::After => "after",
        },
    )
}

/// Workaround for <https://gitlab.computer.surgery/charles/derail/-/issues/5>.
fn fmt_cycle(
    f: &mut fmt::Formatter<'_>,
    scc: &Vec<Arc<Named<config::Process>>>,
) -> fmt::Result {
    if let [node] = &**scc {
        return write!(f, "`{node}` depends on itself");
    }

    write!(f, "the dependencies between ")?;

    for (is_last, node) in
        scc.iter().enumerate().map(|(i, x)| (i + 1 == scc.len(), x))
    {
        if is_last && scc.len() >= 2 {
            write!(f, "and `{node}`")?;
        } else if is_last {
            write!(f, "`{node}`")?;
        } else if scc.len() == 2 {
            write!(f, "`{node}` ")?;
        } else {
            write!(f, "`{node}`, ")?;
        }
    }

    write!(f, " form a cycle")?;

    Ok(())
}

/// The process was not found.
#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
#[derail(
    type Details = Details,
    display("the process `{_0}` was not found"),
    details = Details::empty(),
)]
pub(crate) struct ProcessNotFound(pub(crate) Box<Name>);

/// An error within the Engage file.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum File {
    /// The `command` list of a process was empty.
    #[derail(
        display(
            "`{_0}`'s value for the \"command\" key is an empty list which is \
             not allowed"
        ),
        details = Details::empty()
            .help(
                "either remove the process or, at a minimum, specify the \
                 program to run as the first element in the list",
            ),
    )]
    EmptyCommand(Box<Name>),
}

/// An error type that adds context to a [`Process`].
#[derive(Debug, Error)]
#[derail(
    type Details = Details,
    display("failed to run `{name}`"),
    details = Details::empty(),
)]
pub(crate) struct ProcessContext {
    /// The name of the process that failed.
    pub(crate) name: Box<Name>,

    /// The actual error.
    pub(crate) child: Process,
}

/// Failed to run the graph.
#[derive(Debug, Error)]
#[derail(
    type Details = Details,
    display("{}", RunGraphDisplay(_0.len())),
    details = Details::empty(),
)]
pub(crate) struct RunGraph(#[derail(children)] pub(crate) Vec<ProcessContext>);

/// Workaround for <https://gitlab.computer.surgery/charles/derail/-/issues/5>.
struct RunGraphDisplay(usize);

impl fmt::Display for RunGraphDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 1 {
            write!(f, "failed to run 1 process")
        } else {
            write!(f, "failed to run {} processes", self.0)
        }
    }
}

/// Failed to validate a value for use as a name.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum ValidateName {
    /// The string is empty.
    #[derail(
        display("empty string is not a valid name"),
        details = Details::empty(),
    )]
    Empty,

    /// The starting character is invalid.
    #[derail(
        display(
            "'{}' is not allowed to be the first character of a name",
            _0.escape_debug(),
        ),
        details = Details::empty(),
    )]
    InvalidStart(char),

    /// A continuation character is invalid.
    #[derail(
        display(
            "'{}' at byte {_0} is not allowed in a name",
            _1.escape_debug(),
        ),
        details = Details::empty(),
    )]
    InvalidContinue(usize, char),
}

impl std::error::Error for ValidateName {}
