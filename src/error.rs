//! Error handling facilities.

use std::{fmt, io, process::ExitStatus};

use derail::CoreCompat;
use derail_macros::Error;

use crate::{graph, name::Name};

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

    /// The requested task was not found.
    TaskNotFound(#[derail(skip_self)] TaskNotFound),

    /// Failed to write to `stdout`.
    #[derail(
        display("failed to write to `stdout`"),
        details = Details::empty(),
    )]
    Stdout(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// Failed to run the graph.
    RunGraph(#[derail(skip_self)] RunGraph),
}

#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum LoadConfig {
    /// Failed to find an Engage file.
    #[derail(
        display("failed to find an Engage file"),
        details = Details::empty(),
    )]
    FileFind(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// Failed to canonicalize the given directory.
    #[derail(
        display("failed to canonicalize the given directory"),
        details = Details::empty(),
    )]
    CanonicalizeGiven(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// Failed to read the Engage file.
    #[derail(
        display("failed to read the Engage file"),
        details = Details::empty(),
    )]
    ReadFile(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// The path to the Engage file has no parent directory.
    #[derail(
        display("the path to the Engage file has no parent directory"),
        details = Details::empty(),
    )]
    NoParentDirectory,

    /// Failed to change directories.
    #[derail(
        display("failed to change directories to that of the Engage file"),
        details = Details::empty(),
    )]
    ChangeDirectory(#[derail(child, map_details)] CoreCompat<io::Error>),

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

/// A task failed to run.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum Task {
    /// Failed to spawn the command.
    #[derail(
        display("failed to spawn command \"{}\"", _1.escape_debug()),
        details = Details::empty(),
    )]
    Spawn(#[derail(child, map_details)] CoreCompat<io::Error>, String),

    /// Failed to read the command output.
    #[derail(
        display("failed read command output"),
        details = Details::empty(),
    )]
    Read(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// Failed to wait for the command to exit.
    #[derail(
        display("failed to wait for command to exit"),
        details = Details::empty(),
    )]
    Wait(#[derail(child, map_details)] CoreCompat<io::Error>),

    /// The task failed.
    #[derail(
        display("{_0}"),
        details = Details {
            help: Some("review this task's logs to determine the cause"),
            note: None,
        },
    )]
    ExitStatus(ExitStatus),
}

/// An error building the graph.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum BuildGraph {
    /// An `after` dependency that doesn't exist.
    #[derail(
        display(
            "`{task}` wants to run after `{after}` but the latter does not \
             exist"
        ),
        details = Details {
            help: Some(
                "either create the nonexistent task or remove its name from \
                 the \"after\" list",
            ),
            note: None,
        },
    )]
    AfterNotFound {
        /// The known task.
        task: Box<Name>,

        /// The unknown `after` dependency.
        after: Box<Name>,
    },

    /// A `before` dependency that doesn't exist.
    #[derail(
        display(
            "`{task}` wants to run before `{before}` but the latter does not \
             exist"
        ),
        details = Details {
            help: Some(
                "either create the nonexistent task or remove its name from \
                 the \"before\" list",
            ),
            note: None,
        },
    )]
    BeforeNotFound {
        /// The known task.
        task: Box<Name>,

        /// The unknown `before` dependency.
        before: Box<Name>,
    },
}

/// A cycle in the graph of tasks.
#[derive(Debug, Error)]
#[derail(
    type Details = Details,
    display("{}", CycleDisplay(self)),
    details = Details::empty(),
)]
pub(crate) struct Cycle {
    /// A strongly connected component.
    pub(crate) scc: Vec<graph::Node>,
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

/// The task was not found.
#[derive(Debug, Error)]
#[derail(
    type Details = Details,
    display("no such task `{name}`"),
    details = Details::empty(),
)]
pub(crate) struct TaskNotFound {
    /// The task's name.
    pub(crate) name: Box<Name>,
}

/// An error within the Engage file.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum File {
    /// The `command` list of a task was empty.
    #[derail(
        display("`{_0}`'s command is an empty list which is not allowed"),
        details = Details {
            help: Some(
                "either remove the task or, at a minimum, specify the program \
                 to run as the first element in the list",
            ),
            note: None,
        },
    )]
    EmptyCommand(Box<Name>),
}

/// An error type that adds context to a [`Task`].
#[derive(Debug, Error)]
#[derail(
    type Details = Details,
    display("task `{name}` failed"),
    details = Details::empty(),
)]
pub(crate) struct TaskContext {
    /// The name of the task that failed.
    pub(crate) name: Box<Name>,

    /// The actual error.
    pub(crate) child: Task,
}

/// Failed to run the graph.
#[derive(Debug, Error)]
#[derail(type Details = Details)]
pub(crate) enum RunGraph {
    /// The graph contains cycles.
    #[derail(
        display("refusing to run tasks with dependency cycles"),
        details = Details {
            help: Some(
                "try using `engage dot` to visualize the graph to determine \
                 where to break the cycles",
            ),
            note: Some(
                "running tasks with dependency cycles would result in a \
                 deadlock"
            ),
        },
    )]
    Cyclic(#[derail(children)] Vec<Cycle>),

    /// A task failed while running the graph.
    #[derail(
        display("{}", RunGraphTaskDisplay(_0.len())),
        details = Details::empty(),
    )]
    Task(#[derail(children)] Vec<TaskContext>),
}

/// Workaround for <https://gitlab.computer.surgery/charles/derail/-/issues/5>.
struct RunGraphTaskDisplay(usize);

impl fmt::Display for RunGraphTaskDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 1 {
            write!(f, "failed to run 1 task")
        } else {
            write!(f, "failed to run {} tasks", self.0)
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
