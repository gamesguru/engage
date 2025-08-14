//! Facilities for loading and running tasks.

use std::{env, fmt, io, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::error;

/// Defines the `NAME` static.
macro_rules! define_name {
    ($name:literal) => {
        #[doc = "The canonical name of the Engage file: `"]
        #[doc = $name]
        #[doc = "`\n"]
        /// # Why that name?
        ///
        /// After reading through [this issue][issue] and [this internals
        /// discussion][discussion], the only thing I could decide for sure was
        /// that there should be exactly one allowed form, for the sake of
        /// consistency across projects.
        ///
        /// I'm okay with both the all-lowercase and first-char-uppercase
        /// conventions, because the former is consistent with pretty much
        /// everything else, and the latter stands out, making it easy to spot,
        /// so you know a project uses the tool in question.
        ///
        /// After much indecision and talking with other people about it, a
        /// friend recommended I flip a coin. So I did, and all-lowercase was
        /// chosen first, and won best 2 out of 3, and won best 3 out of 5, in
        /// the same coin-flipping session. So, all-lowercase it is.
        ///
        /// [issue]: https://github.com/rust-lang/cargo/issues/45
        /// [discussion]: https://internals.rust-lang.org/t/can-we-rename-cargo-toml/380
        pub(crate) static NAME: &str = $name;
    };
}

define_name!("engage.toml");

/// Search upwards until an Engage file is found, returning the path to it.
///
/// Does not change the current directory of the calling process, that must be
/// done manually if desired.
///
/// # Errors
///
/// This function can fail:
///
/// * [when determining the current directory][0]
/// * [when looking at files in the current or ancestor directories][1]
/// * if no Engage file is found in the current directory or any of its
///   ancestors.
///
/// [0]: https://doc.rust-lang.org/stable/std/env/fn.current_dir.html#errors
/// [1]: https://doc.rust-lang.org/stable/std/fs/fn.read_dir.html#errors
pub(crate) async fn find() -> io::Result<PathBuf> {
    let mut search_dir = env::current_dir()?;

    loop {
        let mut read_dir = fs::read_dir(&search_dir).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            if entry.file_name() == NAME {
                return Ok(entry.path());
            }
        }

        if !search_dir.pop() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "{NAME} not found in the current directory or its \
                     ancestors"
                ),
            ));
        }
    }
}

/// A task within an Engage file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub(crate) struct Task {
    /// Name of this task.
    pub(crate) name: String,

    /// The group that this task belongs to.
    pub(crate) group: String,

    /// The script to run.
    ///
    /// The string given to this field will be appended to the list given to
    /// the `interpreter` field, and the resulting list will be executed.
    pub(crate) script: String,

    /// Any extra status codes to treat as successful.
    #[serde(rename = "ignore", default)]
    pub(crate) ignored: Vec<i32>,

    /// List of tasks that need to complete before this one can start.
    ///
    /// The values given to this field must be a value of the `name` field of
    /// other tasks within the same group as this task.
    #[serde(default)]
    pub(crate) depends: Vec<String>,
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// A group within an Engage file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub(crate) struct Group {
    /// Name of this group.
    pub(crate) name: String,

    /// List of groups that need to complete before this one can start.
    ///
    /// The values given to this field must be a value of the `group` field
    /// of a task or the `name` field of another group.
    #[serde(default)]
    pub(crate) depends: Vec<String>,
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// An Engage file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub(crate) struct File {
    /// The interpreter that will be used to run task scripts.
    ///
    /// The string given to `script` will be appended to the list given to this
    /// field, and the resulting list will be executed.
    pub(crate) interpreter: Vec<String>,

    /// The list of tasks to run.
    #[serde(default, rename = "task")]
    pub(crate) tasks: Vec<Task>,

    /// Configuration of task groups.
    #[serde(default, rename = "group")]
    pub(crate) groups: Vec<Group>,
}

impl File {
    /// Normalizes the deserialized data.
    ///
    /// Call this function after deserializing, otherwise some things may not
    /// work properly.
    pub(crate) fn normalize(&mut self) {
        for group in self.tasks.iter().map(|x| x.group.as_str()) {
            if self.groups.iter().all(|g| g.name != group) {
                self.groups.push(Group {
                    name: group.to_owned(),
                    depends: Vec::new(),
                });
            }
        }
    }

    /// Validates the configuration file.
    ///
    /// # Errors
    ///
    /// Returns a type describing any errors with the configuration. Errors are
    /// reported on a best-effort basis. For example, fixing all the reported
    /// errors may still result in a different set of errors on the next run.
    pub(crate) fn validate(&self) -> Result<(), Vec<error::File>> {
        let mut errors = Vec::new();

        if self.interpreter.is_empty() {
            errors.push(error::File::EmptyInterpreter);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
