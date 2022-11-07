//! Error handling facilities

use std::{error::Error, fmt, io, iter, process::ExitStatus};

use thiserror::Error;

use crate::{graph, ui};

/// Wraps any [`Error`][e] type so that [`Display`][d] includes its sources
///
/// # Examples
///
/// If `Foo` has a source of `Bar`, and `Bar` has a source of `Baz`, then
/// the formatted output of `Chain(&Foo)` will look like this:
///
/// ```
/// # use engage::error::Chain;
/// # use thiserror::Error;
/// # #[derive(Debug, Error)]
/// # #[error("foo")]
/// # struct Foo(#[from] Bar);
/// # #[derive(Debug, Error)]
/// # #[error("bar")]
/// # struct Bar(#[from] Baz);
/// # #[derive(Debug, Error)]
/// # #[error("baz")]
/// # struct Baz;
/// # fn try_foo() -> Result<(), Foo> { Err(Foo(Bar(Baz))) }
/// match try_foo() {
///     Ok(foo) => {
///         // Do something with foo
///         # drop(foo);
///         # unreachable!()
///     }
///     Err(e) => {
///         assert_eq!(
///             format!("foo error: {}", Chain(&e)),
///             "foo error: foo: bar: baz"
///         );
///     }
/// }
/// ```
///
/// [e]: Error
/// [d]: fmt::Display
#[derive(Debug)]
pub struct Chain<'a>(pub &'a dyn Error);

impl<'a> fmt::Display for Chain<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)?;

        let mut source = self.0.source();

        source
            .into_iter()
            .chain(iter::from_fn(|| {
                source = source.and_then(Error::source);
                source
            }))
            .try_for_each(|source| write!(f, ": {}", source))
    }
}

/// There was an error running the program
#[derive(Debug, Error)]
pub enum Main {
    /// Failed to find an Engage file
    #[error("failed to find an Engage file")]
    FileFind(#[source] io::Error),

    /// Failed to canonicalize the given directory
    #[error("failed to canonicalize the given directory")]
    CanonicalizeGiven(#[source] io::Error),

    /// Failed to read the Engage file
    #[error("failed to read the engage file")]
    ReadFile(#[source] io::Error),

    /// The path to the Engage file has no parent directory
    #[error("the path to the engage file has no parent directory")]
    NoParentDirectory,

    /// Failed to change directories
    #[error("failed to change directories to that of the Engage file")]
    ChangeDirectory(#[source] io::Error),

    /// Failed to deserialize the Engage file
    #[error("failed to deserialize the Engage file")]
    Deserialize(#[from] toml::de::Error),

    /// The Engage file contains errors
    #[error("the Engage file contains errors")]
    File(#[from] Group<File>),

    /// Failed to produce a graph from the Engage file
    #[error("failed to produce a graph from the Engage file")]
    Graph(#[from] Graph),

    /// The requested group or task was not found
    #[error(transparent)]
    NotFound(#[from] NotFound),

    /// Failed to write to `stdout`
    #[error("failed to write to `stdout`")]
    Stdout(#[source] io::Error),

    /// Failed to run the graph
    #[error("failed to run the graph")]
    RunGraph(#[from] RunGraph),
}

/// A task failed to run
#[derive(Debug, Error)]
pub enum Task {
    /// Failed to spawn the command
    #[error("failed to spawn command")]
    Spawn(#[source] io::Error),

    /// Failed to read the command output
    #[error("failed read command output")]
    Read(#[source] io::Error),

    /// Failed to wait for the command to exit
    #[error("failed to wait for command to exit")]
    Wait(#[source] io::Error),

    /// The task failed
    #[error("task failed")]
    ExitStatus(ExitStatus),
}

/// The graph could not be created
#[derive(Debug, Error)]
pub enum Graph {
    /// A task dependends on another task that belongs to a different group
    #[error(
        "dependency task \"{task}\" does not belong to group \
         \"{current_group}\""
    )]
    TaskNotInGroup {
        /// The task being depended upon
        task: String,

        /// The group the current task belongs to
        current_group: String,
    },

    /// A group depends on another group that is not defined
    #[error(
        "group \"{dependency}\", which is a dependency of the group \
         \"{group}\", is not defined"
    )]
    UndefinedGroup {
        /// The group containing the undefined dependency
        group: String,

        /// The undefined dependency
        dependency: String,
    },
}

/// The graph of groups and tasks is not acyclic
#[derive(Debug, Error)]
pub struct Cycle {
    /// A list of pre-formatted strongly connected components
    pub sccs: Vec<Vec<graph::Node>>,
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_string = |node: &graph::Node| match node {
            graph::Node::Task(x) => ui::names_to_prefix(&x.group, &x.name),
            graph::Node::GroupStart(_) | graph::Node::GroupEnd(_) => {
                node.to_string()
            }
        };

        write!(
            f,
            "a dependency cycle is created by the edges between the node \
             set{} ",
            if self.sccs.len() == 1 {
                ""
            } else {
                "s"
            }
        )?;

        for (is_last, scc) in self
            .sccs
            .iter()
            .enumerate()
            .map(|(i, x)| (i + 1 == self.sccs.len(), x))
        {
            let at_least_two = scc.len() >= 2;
            let exactly_two = scc.len() == 2;

            for (is_last, node) in
                scc.iter().enumerate().map(|(i, x)| (i + 1 == scc.len(), x))
            {
                if is_last && at_least_two {
                    write!(f, r#"and "{}""#, to_string(node))?;
                } else if is_last {
                    write!(f, r#""{}""#, to_string(node))?;
                } else if exactly_two {
                    write!(f, r#""{}" "#, to_string(node))?;
                } else {
                    write!(f, r#""{}", "#, to_string(node))?;
                }
            }
            if !is_last {
                write!(f, "; ")?;
            }
        }

        Ok(())
    }
}

/// The requested group or task was not found
#[derive(Debug, Error)]
pub enum NotFound {
    /// A task was not found
    #[error("no such task \"{name}\" in group \"{group}\"")]
    Task {
        /// The task's name
        name: String,

        /// The group that was searched
        group: String,
    },

    /// A group was not found
    #[error("no such group \"{0}\"")]
    Group(String),
}

/// An error within the Engage file
#[derive(Debug, Error)]
pub enum File {
    /// An illegal group name was used
    #[error(r#"illegal group name "{0}""#)]
    IllegalGroupName(String),

    /// The interpreter list was empty
    #[error("`interpreter` must not be an empty list")]
    EmptyInterpreter,
}

/// A group of errors
///
/// The `Display` impl will print each error, seperated by `, `.
#[derive(Debug, Error)]
pub struct Group<E>(pub Vec<E>);

impl<E> fmt::Display for Group<E>
where
    E: std::error::Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (is_last, error) in
            self.0.iter().enumerate().map(|(i, x)| (i + 1 == self.0.len(), x))
        {
            if is_last {
                write!(f, "{}", error)?;
            } else {
                write!(f, "{}, ", error)?;
            }
        }

        Ok(())
    }
}

/// Failed to run the graph
#[derive(Debug, Error)]
pub enum RunGraph {
    /// The graph contains cycles
    #[error("the graph is not acyclic")]
    Cyclic(#[from] Cycle),

    /// A task failed while running the graph
    #[error("task failed")]
    Task(#[from] Task),
}
