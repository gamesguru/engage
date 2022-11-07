//! Facilities for loading and running tasks

use std::{env, fmt, io, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::error;

/// Defines the `NAME` static
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
        /// everything else on sane systems, and the latter stands out, making
        /// it easy to spot, so you know a project uses the tool in question.
        ///
        /// After much indecision and talking with other people about it, a
        /// friend recommended I flip a coin. So I did, and all-lowercase was
        /// chosen first, and won best 2 out of 3, and won best 3 out of 5, in
        /// the same coin-flipping session. So, all-lowercase it is.
        ///
        /// [issue]: https://github.com/rust-lang/cargo/issues/45
        /// [discussion]: https://internals.rust-lang.org/t/can-we-rename-cargo-toml/380
        pub static NAME: &str = $name;
    };
}

define_name!("engage.toml");

/// Things that cannot be used as group names
pub static ILLEGAL_GROUP_NAMES: &[&str] = &["self", "just", "help"];

/// Search upwards until an Engage file is found, returning the path to it
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
pub async fn find() -> io::Result<PathBuf> {
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

/// Representation of the entire Engage file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct File {
    /// The interpreter that'll be used to run task scripts
    pub interpreter: Vec<String>,

    /// The provided tasks
    #[serde(default, rename = "task")]
    pub tasks: Vec<Task>,

    /// Configuration of task groups
    #[serde(default, rename = "group")]
    pub groups: Vec<Group>,
}

impl File {
    /// Normalizes the deserialized data
    ///
    /// Call this function after deserializing, otherwise some things may not
    /// work properly.
    pub fn normalize(&mut self) {
        for group in self.tasks.iter().map(|x| x.group.as_str()) {
            if self.groups.iter().all(|g| g.name != group) {
                self.groups.push(Group {
                    name: group.to_owned(),
                    depends: Vec::new(),
                });
            }
        }
    }

    /// Validates the configuration file
    ///
    /// # Errors
    ///
    /// Returns a type describing any errors with the configuration. Errors are
    /// reported on a best-effort basis. For example, fixing all the reported
    /// errors may still result in a different set of errors on the next run.
    pub fn validate(&self) -> Result<(), error::Group<error::File>> {
        let mut errors = Vec::new();

        if self.interpreter.is_empty() {
            errors.push(error::File::EmptyInterpreter);
        }

        for group in &self.groups {
            if ILLEGAL_GROUP_NAMES.contains(&group.name.as_str()) {
                errors.push(error::File::IllegalGroupName(group.name.clone()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(error::Group(errors))
        }
    }
}
