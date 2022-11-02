//! Tasks and groups thereof

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::TASK_GROUP_NAME_SEPARATOR;

/// A task within the Engage file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Task {
    /// Name of this specific task
    pub name: String,

    /// The group that this task belongs to
    pub group: String,

    /// The script to be executed
    pub script: String,

    /// Any extra status codes to treat as successful
    #[serde(rename = "ignore", default)]
    pub ignored: Vec<i32>,

    /// Other tasks this task depends on
    ///
    /// Tasks must be within the same group.
    #[serde(default)]
    pub depends: Vec<String>,
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Task {
    /// Get the log prefix of this task
    #[must_use]
    pub fn to_prefix(&self) -> String {
        names_to_prefix(&self.group, &self.name)
    }
}

/// A task group within the Engage file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Group {
    /// Name of the group of tasks
    pub name: String,

    /// List of groups that need to run before this one
    #[serde(default)]
    pub depends: Vec<String>,
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Get a unique combination of group and task names
pub(crate) fn names_to_prefix<S1, S2>(group: S1, task: S2) -> String
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    format!("{}{}{}", group.as_ref(), TASK_GROUP_NAME_SEPARATOR, task.as_ref())
}
