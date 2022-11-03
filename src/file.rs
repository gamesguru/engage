//! Facilities for loading and running tasks

use std::{
    env, fmt,
    io::{self, stdout},
    path::PathBuf,
    process::Stdio,
    sync::Arc,
};

use crossterm::{
    execute,
    style::{Attribute, Print, SetAttribute, Stylize},
};
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
};

use crate::{error, OUTPUT_SEPARATOR, TASK_GROUP_NAME_SEPARATOR};

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

/// Distinguish between `stdout` and `stderr`
enum StdKind {
    /// `stdout`
    Out,

    /// `stderr`
    Err,
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
    /// Returns the length of the longest prefix
    #[must_use]
    pub fn longest_prefix(&self) -> usize {
        let mut longest = 0;
        for task in &self.tasks {
            let length = names_to_prefix(&task.group, &task.name).len();

            if length > longest {
                longest = length;
            }
        }

        longest
    }

    /// Try to run a task
    ///
    /// # Errors
    ///
    /// This can fail for a number of reasons, see [`error::Task`][error::Task]
    /// for details.
    pub async fn run_task(
        self: Arc<Self>,
        task: Arc<Task>,
    ) -> Result<(), error::Task> {
        let mut child = Command::new(&self.interpreter[0])
            .args(&self.interpreter[1..])
            .arg(&task.script)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(error::Task::Spawn)?;

        let mut handles = [None, None];

        if let Some(stdout) = child.stdout.take() {
            let s = self.clone();
            handles[0] = Some(tokio::spawn(s.repeat_prefixed(
                StdKind::Out,
                stdout,
                task.clone(),
            )));
        }

        if let Some(stderr) = child.stderr.take() {
            let s = self.clone();
            handles[1] = Some(tokio::spawn(s.repeat_prefixed(
                StdKind::Err,
                stderr,
                task.clone(),
            )));
        }

        for handle in handles.iter_mut().filter_map(Option::take) {
            handle.await.expect("failed to join task")?;
        }

        let status = child.wait().await.map_err(error::Task::Wait)?;

        if !status.success()
            && !status.code().map_or(false, |code| task.ignored.contains(&code))
        {
            return Err(error::Task::ExitStatus(status));
        }

        Ok(())
    }

    /// Repeats the output from the `reader` prefixed with the task info
    async fn repeat_prefixed<R>(
        self: Arc<Self>,
        kind: StdKind,
        reader: R,
        task: Arc<Task>,
    ) -> Result<(), error::Task>
    where
        R: AsyncRead + Unpin,
    {
        let buf_reader = BufReader::new(reader);
        let mut lines = buf_reader.lines();
        let longest_prefix = self.longest_prefix();

        loop {
            let line = lines.next_line().await.map_err(error::Task::Read)?;

            if let Some(line) = line {
                let mut stdout = stdout().lock();

                execute!(
                    stdout,
                    SetAttribute(Attribute::Reset),
                    Print(format!(
                        "{:>width$} ",
                        task.to_prefix(),
                        width = longest_prefix,
                    )),
                    Print(match kind {
                        StdKind::Out => OUTPUT_SEPARATOR.green(),
                        StdKind::Err => OUTPUT_SEPARATOR.red(),
                    }),
                    Print(format!(" {}", line)),
                    Print('\n'),
                )
                .expect("failed to write output");
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Updates the list of groups with any groups not explicitly declared
    ///
    /// Call this function after deserializing, otherwise not all groups will be
    /// noticed.
    pub fn update_groups(&mut self) {
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
    pub fn validate(&self) -> Result<(), error::FileGroup> {
        let mut errors = Vec::new();

        for group in &self.groups {
            if ILLEGAL_GROUP_NAMES.contains(&group.name.as_str()) {
                errors.push(error::File::IllegalGroupName(group.name.clone()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(error::FileGroup {
                errors,
            })
        }
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
