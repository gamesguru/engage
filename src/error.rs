//! Error handling facilities

use std::{error::Error, fmt, io, iter, process::ExitStatus};

use crossterm::{
    execute,
    style::{Print, Stylize},
};
use thiserror::Error;

use crate::{file::names_to_prefix, graph};

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

/// Formats an error message to be printed on the command line
///
/// The returned string includes a trailing newline.
#[must_use]
pub fn format_cli<D>(error: D) -> String
where
    D: fmt::Display,
{
    let mut buf = Vec::new();

    execute!(
        buf,
        Print("error".red().bold()),
        Print(':'.bold()),
        Print(' '),
        Print(error),
        Print('\n'),
    )
    .expect("should be able to write to in-memory buffer");

    String::from_utf8(buf).expect("should be a valid UTF-8 string")
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
    pub(crate) sccs: Vec<Vec<graph::Node>>,
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_string = |node: &graph::Node| match node {
            graph::Node::Task(x) => names_to_prefix(&x.group, &x.name),
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
}

/// A group of errors within the Engage file
#[derive(Debug, Error)]
pub struct FileGroup {
    /// The inner list of errors
    pub(crate) errors: Vec<File>,
}

impl FileGroup {
    /// Get an iterator over the individual errors
    pub fn errors(&self) -> impl Iterator<Item = &File> {
        self.errors.iter()
    }
}

impl fmt::Display for FileGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "errors are present in the configuration: ")?;

        for (is_last, error) in self
            .errors
            .iter()
            .enumerate()
            .map(|(i, x)| (i + 1 == self.errors.len(), x))
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
