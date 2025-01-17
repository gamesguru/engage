//! Error handling facilities.

use std::{fmt, io, process::ExitStatus};

use derail::CoreCompat;
use derail_macros::Error;

use crate::{graph, ui};

/// There was an error running the program.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum Main {
    /// Failed to find an Engage file.
    #[derail(display("failed to find an Engage file"))]
    FileFind(#[derail(child)] CoreCompat<io::Error>),

    /// Failed to canonicalize the given directory.
    #[derail(display("failed to canonicalize the given directory"))]
    CanonicalizeGiven(#[derail(child)] CoreCompat<io::Error>),

    /// Failed to read the Engage file.
    #[derail(display("failed to read the engage file"))]
    ReadFile(#[derail(child)] CoreCompat<io::Error>),

    /// The path to the Engage file has no parent directory.
    #[derail(display("the path to the engage file has no parent directory"))]
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

    /// Failed to produce a graph from the Engage file.
    #[derail(display("failed to produce a graph from the Engage file"))]
    Graph(#[derail(child)] Graph),

    /// The requested group or task was not found.
    NotFound(#[derail(skip_self)] NotFound),

    /// Failed to write to `stdout`.
    #[derail(display("failed to write to `stdout`"))]
    Stdout(#[derail(child)] CoreCompat<io::Error>),

    /// Failed to run the graph.
    #[derail(display("failed to run the graph"))]
    RunGraph(#[derail(child)] RunGraph),
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

/// The graph could not be created.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum Graph {
    /// A task dependends on another task that belongs to a different group.
    #[derail(display(
        "dependency task \"{task}\" does not belong to group \
         \"{current_group}\""
    ))]
    TaskNotInGroup {
        /// The task being depended upon.
        task: String,

        /// The group the current task belongs to.
        current_group: String,
    },

    /// A group depends on another group that is not defined.
    #[derail(display(
        "group \"{dependency}\", which is a dependency of the group \
         \"{group}\", is not defined"
    ))]
    UndefinedGroup {
        /// The group containing the undefined dependency.
        group: String,

        /// The undefined dependency.
        dependency: String,
    },
}

/// A cycle in the graph of groups and tasks.
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
        let to_string = |node: &graph::Node| match node {
            graph::Node::Task(x) => ui::names_to_prefix(&x.group, &x.name),
            graph::Node::GroupStart(_) | graph::Node::GroupEnd(_) => {
                node.to_string()
            }
        };

        let scc = &self.0.scc;

        if let [node] = &**scc {
            return write!(f, r#""{}" depends on itself"#, to_string(node));
        }

        write!(f, "the dependencies between ")?;

        for (is_last, node) in
            scc.iter().enumerate().map(|(i, x)| (i + 1 == scc.len(), x))
        {
            if is_last && scc.len() >= 2 {
                write!(f, r#"and "{}""#, to_string(node))?;
            } else if is_last {
                write!(f, r#""{}""#, to_string(node))?;
            } else if scc.len() == 2 {
                write!(f, r#""{}" "#, to_string(node))?;
            } else {
                write!(f, r#""{}", "#, to_string(node))?;
            }
        }

        write!(f, " form a cycle")?;

        Ok(())
    }
}

/// The requested group or task was not found.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum NotFound {
    /// A task was not found.
    #[derail(display("no such task \"{name}\" in group \"{group}\""))]
    Task {
        /// The task's name.
        name: String,

        /// The group that was searched.
        group: String,
    },

    /// A group was not found.
    #[derail(display("no such group \"{_0}\""))]
    Group(String),
}

/// An error within the Engage file.
#[derive(Debug, Error)]
#[derail(type Details = ())]
pub(crate) enum File {
    /// The interpreter list was empty.
    #[derail(display("`interpreter` must not be an empty list"))]
    EmptyInterpreter,
}

/// An error type that adds context to a [`Task`].
#[derive(Debug, Error)]
#[derail(
    type Details = (),
    display("{}", ui::names_to_prefix(group, task)),
)]
pub(crate) struct TaskContext {
    /// The name of the task that failed.
    pub(crate) task: String,

    /// The name of the group the failed task is in.
    pub(crate) group: String,

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
