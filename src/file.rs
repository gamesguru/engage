//! Facilities for loading and running tasks

use std::{io::stdout, process::Stdio, sync::Arc};

use crossterm::{
    execute,
    style::{Attribute, Print, SetAttribute, Stylize},
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
};

use crate::{
    error, task::names_to_prefix, Group, Task, ILLEGAL_GROUP_NAMES,
    OUTPUT_SEPARATOR,
};

/// Distinguish between `stdout` and `stderr`
enum StdKind {
    /// `stdout`
    Out,

    /// `stderr`
    Err,
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
