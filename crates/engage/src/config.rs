//! Configuration.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    path::{Path, PathBuf},
};

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

/// Parsed content of a configuration file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// The processes to run.
    #[serde(default)]
    pub(crate) processes: BTreeMap<Box<Name>, Process>,
}

impl Config {
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

        for (name, process) in &self.processes {
            if process.command.is_empty() {
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

/// When a process is considered to be ready.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ReadyWhen {
    /// The process is considered ready when it has exited.
    Exited,

    /// The process is considered ready when it has spawned.
    Spawned,
}

/// A process within an Engage file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Process {
    /// The command used to spawn the process.
    pub(crate) command: Vec<String>,

    /// When the process should be considered ready.
    pub(crate) ready_when: ReadyWhen,

    /// Extra environment variables to set for the process.
    ///
    /// Values provided here will take precedence over any ambient environment
    /// variable of the same name.
    #[serde(default)]
    pub(crate) environment: BTreeMap<String, String>,

    /// List of names of processes that need to exit successfully before this
    /// one can be spawned.
    #[serde(default)]
    pub(crate) after: BTreeSet<Box<Name>>,

    /// List of names of processes that can be spawned only after this one has
    /// exited successfully.
    #[serde(default)]
    pub(crate) before: BTreeSet<Box<Name>>,
}

/// Search upwards until `engage.toml` is found, returning the path to it.
///
/// # Errors
///
/// This function can fail when:
///
/// * [Looking at files in the current directory or ancestor directories][0].
/// * No `engage.toml` is found in the current directory or any of its
///   ancestors.
///
/// [0]: https://doc.rust-lang.org/stable/std/fs/fn.read_dir.html#errors
pub(crate) async fn find<P>(search_dir: P) -> Result<PathBuf, error::FileFind>
where
    P: AsRef<Path>,
{
    use error::FileFind as E;

    let mut search_dir = search_dir.as_ref();
    loop {
        let mut read_dir = fs::read_dir(&search_dir)
            .await
            .map_err(|e| E::ReadDir(e.into(), search_dir.to_owned()))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| E::NextEntry(e.into(), search_dir.to_owned()))?
        {
            if entry.file_name() == DEFAULT_FILE_NAME {
                return Ok(entry.path());
            }
        }

        search_dir = if let Some(x) = search_dir.parent() {
            x
        } else {
            return Err(E::NotFound);
        };
    }
}

/// Attempt to load an Engage file, returning its contents and absolute parent
/// directory.
pub(crate) async fn load<P>(
    file: Option<P>,
) -> Result<(Config, PathBuf), error::LoadConfig>
where
    P: AsRef<Path>,
{
    use error::LoadConfig as E;

    let current_dir =
        env::current_dir().map_err(|e| E::CurrentDir(e.into()))?;

    let found;
    let file = match &file {
        None => {
            found = find(&current_dir).await.map_err(E::FileFind)?;
            &*found
        }
        Some(file) => file.as_ref(),
    };

    let content =
        fs::read_to_string(&file).await.map_err(|e| E::ReadFile(e.into()))?;

    let config = toml::from_str::<Config>(&content)
        .map_err(|e| E::Deserialize(e.into()))?;

    config.validate().map_err(E::File)?;

    // Reading the file should fail first in cases where this would fail.
    let parent = file.parent().expect("a file should have a parent directory");

    Ok((config, current_dir.join(parent)))
}
