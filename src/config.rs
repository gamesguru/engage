//! Configuration.

use std::{env, fmt, io, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{cli, error};

/// The default file name, `engage.toml`.
///
/// # Why that name?
///
/// After reading through [this issue][0] and [this internals discussion][1],
/// the only thing I could decide for sure was that there should be exactly one
/// allowed form, for the sake of consistency across projects.
///
/// I'm okay with both the all-lowercase and first-char-uppercase conventions,
/// because the former is consistent with pretty much everything else, and the
/// latter stands out, making it easy to spot, so you know a project uses the
/// tool in question.
///
/// After much indecision and talking with other people about it, a friend
/// recommended I flip a coin. So I did, and all-lowercase was chosen first,
/// and won best 2 out of 3, and won best 3 out of 5, in the same coin-flipping
/// session. So, all-lowercase it is.
///
/// [0]: https://github.com/rust-lang/cargo/issues/45
/// [1]: https://internals.rust-lang.org/t/can-we-rename-cargo-toml/380
pub(crate) static DEFAULT_FILE_NAME: &str = "engage.toml";

/// Parsed content of a configuration file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub(crate) struct Config {
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

impl Config {
    /// Normalizes the parsed data.
    ///
    /// Call this function after parsing, otherwise some things may not
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

/// Search upwards until `engage.toml` is found, returning the path to it.
///
/// # Errors
///
/// This function can fail when:
///
/// * [Determining the current directory][0].
/// * [Looking at files in the current directory or ancestor directories][1].
/// * No `engage.toml` is found in the current directory or any of its
///   ancestors.
///
/// [0]: https://doc.rust-lang.org/stable/std/env/fn.current_dir.html#errors
/// [1]: https://doc.rust-lang.org/stable/std/fs/fn.read_dir.html#errors
pub(crate) async fn find() -> io::Result<PathBuf> {
    let mut search_dir = env::current_dir()?;

    loop {
        let mut read_dir = fs::read_dir(&search_dir).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            if entry.file_name() == DEFAULT_FILE_NAME {
                return Ok(entry.path());
            }
        }

        if !search_dir.pop() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "{DEFAULT_FILE_NAME} not found in the current directory \
                     or its ancestors"
                ),
            ));
        }
    }
}

/// Attempt to load an Engage file.
pub(crate) async fn load(
    args: &cli::Args,
) -> Result<Config, error::LoadConfig> {
    use error::LoadConfig as Error;

    let file = match &args.file {
        None => find().await.map_err(|e| Error::FileFind(e.into()))?,
        Some(file) => file
            .canonicalize()
            .map_err(|e| Error::CanonicalizeGiven(e.into()))?,
    };

    env::set_current_dir(file.parent().ok_or(Error::NoParentDirectory)?)
        .map_err(|e| Error::ChangeDirectory(e.into()))?;

    let content = fs::read_to_string(file)
        .await
        .map_err(|e| Error::ReadFile(e.into()))?;

    let mut config = toml::from_str::<Config>(&content)
        .map_err(|e| Error::Deserialize(e.into()))?;

    config.normalize();
    config.validate().map_err(Error::File)?;

    Ok(config)
}
