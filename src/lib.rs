#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![warn(clippy::as_conversions)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::empty_structs_with_brackets)]
#![warn(clippy::get_unwrap)]
#![warn(clippy::if_then_some_else_none)]
#![warn(clippy::let_underscore_must_use)]
#![warn(clippy::map_err_ignore)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::negative_feature_names)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::rc_mutex)]
#![warn(clippy::redundant_feature_names)]
#![warn(clippy::rest_pat_in_fully_bound_structs)]
#![warn(clippy::str_to_string)]
#![warn(clippy::string_add)]
#![warn(clippy::string_slice)]
#![warn(clippy::string_to_string)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(clippy::unseparated_literal_suffix)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::wildcard_dependencies)]

use std::{
    io::{self, stdout},
    process::{ExitStatus, Stdio},
    sync::Arc,
};

use crossterm::{
    execute,
    style::{Print, Stylize},
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
};

pub mod error;

/// The separator between the task group and name
const PREFIX_SEPARATOR: &str = "::";

/// Representation of the entire `engage.toml` file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Engage {
    /// The shell that'll be used to run commands
    pub shell: Vec<String>,

    /// The tasks provided by the `engage.toml` file
    pub task: Vec<Task>,
}

/// A task within `engage.toml`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Task {
    /// Name of this specific task
    pub name: String,

    /// The group that this task belongs to
    pub group: String,

    /// The command to be executed
    pub cmd: String,

    /// Any extra status codes to treat as successful
    #[serde(default)]
    pub ignored: Vec<i32>,
}

impl Engage {
    /// Returns the length of the longest prefix
    #[must_use]
    pub fn longest_prefix(&self) -> usize {
        let mut longest = 0;
        for task in &self.task {
            let length =
                task.group.len() + task.name.len() + PREFIX_SEPARATOR.len();

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
    /// This can fail for a number of reasons, see [`TaskError`][TaskError] for
    /// details.
    pub async fn run_task(
        self: Arc<Self>,
        task: Arc<Task>,
    ) -> Result<(), TaskError> {
        let mut child = Command::new(&self.shell[0])
            .args(&self.shell[1..])
            .arg(&task.cmd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(TaskError::Spawn)?;

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

        let status = child.wait().await.map_err(TaskError::Wait)?;

        if !status.success()
            && !status.code().map_or(false, |code| task.ignored.contains(&code))
        {
            return Err(TaskError::ExitStatus(status));
        }

        Ok(())
    }

    /// Repeats the output from the `reader` prefixed with the task info
    async fn repeat_prefixed<R>(
        self: Arc<Self>,
        kind: StdKind,
        reader: R,
        task: Arc<Task>,
    ) -> Result<(), TaskError>
    where
        R: AsyncRead + Unpin,
    {
        let buf_reader = BufReader::new(reader);
        let mut lines = buf_reader.lines();

        loop {
            let line = lines.next_line().await.map_err(TaskError::Read)?;

            if let Some(line) = line {
                let mut stdout = stdout();

                execute!(
                    stdout,
                    Print(format!(
                        "{:>width$} ",
                        task.to_prefix(),
                        width = self.longest_prefix()
                    )),
                    Print(match kind {
                        StdKind::Out => "│".green(),
                        StdKind::Err => "│".red(),
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
}

impl Task {
    /// Get the log prefix of this task
    #[must_use]
    pub fn to_prefix(&self) -> String {
        format!("{}{}{}", self.group, PREFIX_SEPARATOR, self.name)
    }
}

/// Errors that can occur while trying to run a task
#[derive(thiserror::Error, Debug)]
pub enum TaskError {
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

/// Distinguish between `stdout` and `stderr`
enum StdKind {
    /// `stdout`
    Out,

    /// `stderr`
    Err,
}
