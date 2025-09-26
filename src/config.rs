//! Configuration.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, io,
    path::{Path, PathBuf},
};

use semver::Version;
use serde::Deserialize;
use tokio::fs;

use crate::{error, name::Name};

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

/// Extract the version from a configuration file.
#[derive(Deserialize)]
struct ExtractVersionReq {
    /// Version requirement of the configuration file.
    version: Version,
}

/// Parsed content of a configuration file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Version requirement of the configuration file.
    #[expect(
        dead_code,
        reason = "only present to avoid denying it as unknown"
    )]
    pub(crate) version: Version,

    /// The tasks to run.
    #[serde(default)]
    pub(crate) tasks: BTreeMap<Box<Name>, Task>,
}

impl Config {
    /// Get the version of this configuration format.
    fn version() -> Version {
        Version::parse("0.0.0-dev").expect("hard-coded version should be valid")
    }

    /// Validates the configuration file.
    ///
    /// # Errors
    ///
    /// Returns a type describing any errors with the configuration. Errors are
    /// reported on a best-effort basis. For example, fixing all the reported
    /// errors may still result in a different set of errors on the next run.
    fn validate(&self) -> Result<(), Vec<error::File>> {
        use error::File as E;

        let mut errors = Vec::new();

        for (name, task) in &self.tasks {
            if task.command.is_empty() {
                errors.push(E::EmptyCommand(name.clone()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// A task within an Engage file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Task {
    /// The command to run.
    pub(crate) command: Vec<String>,

    /// Extra environment variables to set when running `command`.
    ///
    /// Values provided here will take precedence over any ambient environment
    /// variable of the same name.
    #[serde(default)]
    pub(crate) environment: BTreeMap<String, String>,

    /// List of tasks that need to complete before this one can start.
    ///
    /// The values given to this field must be equal to the name of other
    /// tasks.
    #[serde(default)]
    pub(crate) after: BTreeSet<Box<Name>>,

    /// List of tasks that will be started only after this task is complete.
    ///
    /// The values given to this field must be equal to the name of other
    /// tasks.
    #[serde(default)]
    pub(crate) before: BTreeSet<Box<Name>>,
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
pub(crate) async fn load<P>(
    file: Option<P>,
) -> Result<Config, error::LoadConfig>
where
    P: AsRef<Path>,
{
    use error::LoadConfig as E;

    let file = match file {
        None => find().await.map_err(|e| E::FileFind(e.into()))?,
        Some(file) => file
            .as_ref()
            .canonicalize()
            .map_err(|e| E::CanonicalizeGiven(e.into()))?,
    };

    env::set_current_dir(file.parent().ok_or(E::NoParentDirectory)?)
        .map_err(|e| E::ChangeDirectory(e.into()))?;

    let content =
        fs::read_to_string(file).await.map_err(|e| E::ReadFile(e.into()))?;

    let version = toml::from_str::<ExtractVersionReq>(&content)
        .map_err(|e| E::Deserialize(e.into()))?
        .version;

    if !crate::semver::compatible(&version, &Config::version()) {
        return Err(E::VersionsIncompatible(version, Config::version()));
    }

    let config = toml::from_str::<Config>(&content)
        .map_err(|e| E::Deserialize(e.into()))?;

    config.validate().map_err(E::File)?;

    Ok(config)
}
