//! Types used for working with the graph of tasks and groups thereof

use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{Group, Task};

/// Errors that can occur when producing a DAG of groups and tasks
#[derive(thiserror::Error, Debug)]
pub enum Error {
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

    /// Groups and tasks were not acyclic
    #[error("dependency cycle detected")]
    Cycle,
}

/// A node in the dependency graph of tasks and groups
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Node {
    /// The beginning of a group's execution
    GroupStart(Group),

    /// A task
    Task(Task),

    /// The end of a group's execution
    GroupEnd,
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::GroupStart(start) => write!(f, "group start: {}", start),
            Node::Task(task) => write!(f, "task: {}", task),
            Node::GroupEnd => write!(f, "group end"),
        }
    }
}
