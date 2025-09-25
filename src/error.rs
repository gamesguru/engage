//! Error handling facilities.

use std::{fmt, io, process::ExitStatus};

use derail::CoreCompat;
use derail_macros::Error;

use crate::{graph, name::Name};

/// There was an error running the program.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum Main {
    /// Aborted due to command line usage.
    #[derail(display("aborted due to command line usage"))]
    Cli,

    /// Failed to load the Engage file.
    #[derail(display("failed to load the Engage file"))]
    LoadConfig(#[derail(child)] LoadConfig),

    /// Failed to build a graph from the Engage file.
    #[derail(display("failed to build a graph from the Engage file"))]
    BuildGraph(#[derail(children)] Vec<BuildGraph>),

    /// The requested task was not found.
    TaskNotFound(#[derail(skip_self)] TaskNotFound),

    /// Failed to write to `stdout`.
    #[derail(display("failed to write to `stdout`"))]
    Stdout(#[derail(child)] CoreCompat<io::Error>),

    /// Failed to run the graph.
    #[derail(display("failed to run the graph"))]
    RunGraph(#[derail(child)] RunGraph),
}

#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum LoadConfig {
    /// Failed to find an Engage file.
    #[derail(display("failed to find an Engage file"))]
    FileFind(#[derail(child)] CoreCompat<io::Error>),

    /// Failed to canonicalize the given directory.
    #[derail(display("failed to canonicalize the given directory"))]
    CanonicalizeGiven(#[derail(child)] CoreCompat<io::Error>),

    /// Failed to read the Engage file.
    #[derail(display("failed to read the Engage file"))]
    ReadFile(#[derail(child)] CoreCompat<io::Error>),

    /// The path to the Engage file has no parent directory.
    #[derail(display("the path to the Engage file has no parent directory"))]
    NoParentDirectory,

    /// Failed to change directories.
    #[derail(display(
        "failed to change directories to that of the Engage file"
    ))]
    ChangeDirectory(#[derail(child)] CoreCompat<io::Error>),

    /// Failed to deserialize the Engage file.
    #[derail(display("failed to deserialize the Engage file"))]
    Deserialize(#[derail(child)] CoreCompat<toml::de::Error>),

    /// The Engage file contains errors.
    #[derail(display("the Engage file contains errors"))]
    File(#[derail(children)] Vec<File>),
}

/// A task failed to run.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum Task {
    /// Failed to spawn the command.
    #[derail(display("failed to spawn command \"{_1}\""))]
    Spawn(#[derail(child)] CoreCompat<io::Error>, String),

    /// Failed to read the command output.
    #[derail(display("failed read command output"))]
    Read(#[derail(child)] CoreCompat<io::Error>),

    /// Failed to wait for the command to exit.
    #[derail(display("failed to wait for command to exit"))]
    Wait(#[derail(child)] CoreCompat<io::Error>),

    /// The task failed.
    #[derail(display("{_0}"))]
    ExitStatus(ExitStatus),
}

/// An error building the graph.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum BuildGraph {
    /// An `after` dependency that doesn't exist.
    #[derail(display(
        "\"{task}\" wants to run after \"{after}\" but the latter does not \
         exist"
    ))]
    AfterNotFound {
        /// The known task.
        task: Box<Name>,

        /// The unknown `after` dependency.
        after: Box<Name>,
    },

    /// A `before` dependency that doesn't exist.
    #[derail(display(
        "\"{task}\" wants to run before \"{before}\" but the latter does not \
         exist"
    ))]
    BeforeNotFound {
        /// The known task.
        task: Box<Name>,

        /// The unknown `before` dependency.
        before: Box<Name>,
    },
}

/// A cycle in the graph of tasks.
#[derive(Debug, Error)]
#[derail(type Details = (), display("{}", CycleDisplay(self)))]
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
            return write!(f, r#""{node}" depends on itself"#);
        }

        write!(f, "the dependencies between ")?;

        for (is_last, node) in
            scc.iter().enumerate().map(|(i, x)| (i + 1 == scc.len(), x))
        {
            if is_last && scc.len() >= 2 {
                write!(f, r#"and "{node}""#)?;
            } else if is_last {
                write!(f, r#""{node}""#)?;
            } else if scc.len() == 2 {
                write!(f, r#""{node}" "#)?;
            } else {
                write!(f, r#""{node}", "#)?;
            }
        }

        write!(f, " form a cycle")?;

        Ok(())
    }
}

/// The task was not found.
#[derive(Debug, Error)]
#[derail(
    type Details = (),
    display("no such task \"{name}\""),
)]
pub(crate) struct TaskNotFound {
    /// The task's name.
    pub(crate) name: Box<Name>,
}

/// An error within the Engage file.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum File {
    /// The `command` list of a task was empty.
    #[derail(display(
        "\"{_0}\"'s `command` is an empty list which is not allowed"
    ))]
    EmptyCommand(Box<Name>),
}

/// An error type that adds context to a [`Task`].
#[derive(Debug, Error)]
#[derail(
    type Details = (),
    display("{name}"),
)]
pub(crate) struct TaskContext {
    /// The name of the task that failed.
    pub(crate) name: Box<Name>,

    /// The actual error.
    pub(crate) child: Task,
}

/// Failed to run the graph.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum RunGraph {
    /// The graph contains cycles.
    #[derail(display("the graph is not acyclic"))]
    Cyclic(#[derail(children)] Vec<Cycle>),

    /// A task failed while running the graph.
    #[derail(display("one or more tasks failed"))]
    Task(#[derail(children)] Vec<TaskContext>),
}

/// Failed to validate a value for use as a name.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum ValidateName {
    /// The string is empty.
    #[derail(display("empty string is not a valid name"))]
    Empty,

    /// The starting character is invalid.
    #[derail(display(
        "'{_0}' is not allowed to be the first character of a name"
    ))]
    InvalidStart(char),

    /// A continuation character is invalid.
    #[derail(display("'{_1}' at byte {_0} is not allowed in a name"))]
    InvalidContinue(usize, char),
}

impl std::error::Error for ValidateName {}
