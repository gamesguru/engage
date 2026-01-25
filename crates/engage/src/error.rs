//! Error handling facilities.

use std::{fmt, io, path::PathBuf, process::ExitStatus, sync::Arc};

use derail::CoreCompat;
use derail_macros::Error;
use nix::errno::Errno;
use tracing_subscriber::filter::FromEnvError;

use crate::{
    config,
    name::{Name, Named},
};

pub(crate) mod report;

/// Error details.
pub(crate) struct Details {
    /// A recommendation for resolving the error.
    help: Option<&'static str>,

    /// A note about the error.
    note: Option<&'static str>,
}

impl Details {
    /// An empty collection of details.
    fn empty() -> Self {
        Details {
            help: None,
            note: None,
        }
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

    /// Failed to build a graph from the Engage file.
    #[derail(
        display("failed to build a graph from the Engage file"),
        details = Details::empty(),
    )]
    BuildGraph(#[derail(children)] Vec<BuildGraph>),

    /// The requested process was not found.
    ProcessNotFound(#[derail(skip_self)] ProcessNotFound),

    /// The graph contains cycles.
    #[derail(
        display("refusing to run processes with dependency cycles"),
        details = Details {
            help: Some(
                "try using `engage dot` to visualize the graph to determine \
                 where to break the cycles",
            ),
            note: Some(
                "running processes with dependency cycles would result in a \
                 deadlock"
            ),
        },
    )]
    Cyclic(#[derail(children)] Vec<Cycle>),

    /// Failed to write to `stdout`.
    #[derail(
        display("failed to write to `stdout`"),
        details = Details::empty(),
    )]
    Stdout(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// Failed to run the graph.
    RunGraph(#[derail(skip_self)] RunGraph),

    /// Failed to initialize observability.
    Observability(#[derail(skip_self)] Observability),
}

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
        details = Details {
            help: Some(
                "double check the current directory or specify the Engage file \
                 on the command line"
            ),
            note: None,
        },
    )]
    NotFound,
}

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
        details = Details {
            help: Some("review this process' logs to determine the cause"),
            note: None,
        },
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
    /// An `after` dependency that doesn't exist.
    #[derail(
        display(
            "`{process}` wants to run after `{after}` but the latter does not \
             exist"
        ),
        details = Details {
            help: Some(
                "either create the nonexistent process or remove its name from \
                 the \"after\" list",
            ),
            note: None,
        },
    )]
    AfterNotFound {
        /// The known process.
        process: Box<Name>,

        /// The unknown `after` dependency.
        after: Box<Name>,
    },

    /// A `before` dependency that doesn't exist.
    #[derail(
        display(
            "`{process}` wants to run before `{before}` but the latter does \
             not exist"
        ),
        details = Details {
            help: Some(
                "either create the nonexistent process or remove its name from \
                 the \"before\" list",
            ),
            note: None,
        },
    )]
    BeforeNotFound {
        /// The known process.
        process: Box<Name>,

        /// The unknown `before` dependency.
        before: Box<Name>,
    },
}

/// A cycle in the graph.
#[derive(Debug, Error)]
#[derail(
    type Details = Details,
    display("{}", CycleDisplay(self)),
    details = Details::empty(),
)]
pub(crate) struct Cycle {
    /// A strongly connected component.
    pub(crate) scc: Vec<Arc<Named<config::Process>>>,
}

/// Workaround for <https://gitlab.computer.surgery/charles/derail/-/issues/5>.
struct CycleDisplay<'a>(&'a Cycle);

impl fmt::Display for CycleDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scc = &self.0.scc;

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
}

/// The process was not found.
#[derive(Debug, Error)]
#[derail(
    type Details = Details,
    display("no such process `{name}`"),
    details = Details::empty(),
)]
pub(crate) struct ProcessNotFound {
    /// The process' name.
    pub(crate) name: Box<Name>,
}

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
        details = Details {
            help: Some(
                "either remove the process or, at a minimum, specify the \
                 program to run as the first element in the list",
            ),
            note: None,
        },
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
