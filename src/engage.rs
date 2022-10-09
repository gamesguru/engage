//! Facilities for loading and running tasks

use std::{collections::HashMap, io::stdout, process::Stdio, sync::Arc};

use crossterm::{
    execute,
    style::{Attribute, Print, SetAttribute, Stylize},
};
use petgraph::{algo::is_cyclic_directed, prelude::DiGraph};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
};

use crate::{task::names_to_prefix, GraphError, Group, Node, Task, TaskError};

/// Distinguish between `stdout` and `stderr`
enum StdKind {
    /// `stdout`
    Out,

    /// `stderr`
    Err,
}

/// Representation of the entire Engage file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Engage {
    /// The interpreter that'll be used to run task scripts
    pub interpreter: Vec<String>,

    /// The provided tasks
    #[serde(default, rename = "task")]
    pub tasks: Vec<Task>,

    /// Configuration of task groups
    #[serde(default, rename = "group")]
    pub groups: Vec<Group>,
}

impl Engage {
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
    /// This can fail for a number of reasons, see [`TaskError`][TaskError] for
    /// details.
    pub async fn run_task(
        self: Arc<Self>,
        task: Arc<Task>,
    ) -> Result<(), TaskError> {
        let mut child = Command::new(&self.interpreter[0])
            .args(&self.interpreter[1..])
            .arg(&task.script)
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
        let longest_prefix = self.longest_prefix();

        loop {
            let line = lines.next_line().await.map_err(TaskError::Read)?;

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

    /// Get a DAG of the groups and tasks to be executed
    ///
    /// # Errors
    ///
    /// See the variants of [`GraphError`][GraphError] for why this function
    /// might fail.
    pub fn to_graph(&self) -> Result<DiGraph<Node, u32>, GraphError> {
        let mut graph = DiGraph::new();

        // TODO: something more correct than this
        let mut group_to_index = HashMap::new();
        let mut task_to_index = HashMap::new();
        let mut task_to_group = HashMap::new();

        // Add all the nodes
        for group in self.groups.iter().cloned() {
            // Add group nodes
            let group_start_index =
                graph.add_node(Node::GroupStart(group.clone()));
            let group_end_index = graph.add_node(Node::GroupEnd);

            group_to_index.insert(
                group.name.clone(),
                (group_start_index, group_end_index),
            );

            let tasks = self.tasks.iter().filter(|t| t.group == group.name);

            // Add task nodes and an edge to its group
            for task in tasks.clone() {
                let task_index = graph.add_node(Node::Task(task.clone()));
                graph.add_edge(group_start_index, task_index, 1);
                graph.add_edge(task_index, group_end_index, 1);
                task_to_index.insert(task.to_prefix(), task_index);
                task_to_group.insert(
                    (group.name.clone(), task.name.clone()),
                    group.name.clone(),
                );
            }

            // Go back through the tasks to add edges for task dependencies
            for task in tasks {
                let task_index =
                    if let Some(x) = task_to_index.get(&task.to_prefix()) {
                        *x
                    } else {
                        continue;
                    };

                for dep in task.depends.iter().map(String::as_str) {
                    let dep_index = task_to_group
                        .get(&(group.name.clone(), dep.to_owned()))
                        .filter(|g| g.as_str() == group.name.as_str())
                        .and_then(|g| {
                            task_to_index.get(&names_to_prefix(g, dep))
                        });

                    let dep_index = match dep_index {
                        Some(x) => *x,
                        None => {
                            return Err(GraphError::TaskNotInGroup {
                                task: dep.to_owned(),
                                current_group: group.name.clone(),
                            })
                        }
                    };

                    // Require the dependency to be completed before this
                    graph.add_edge(dep_index, task_index, 1);

                    // Remove redundant incoming edge, if any
                    if let Some(group_start_edge) =
                        graph.find_edge(group_start_index, task_index)
                    {
                        graph.remove_edge(group_start_edge);
                    }

                    // Remove redundant outgoing edge, if any
                    if let Some(group_end_edge) =
                        graph.find_edge(dep_index, group_end_index)
                    {
                        graph.remove_edge(group_end_edge);
                    }
                }
            }
        }

        // Add the group edges, if any
        for group in &self.groups {
            for depend in &group.depends {
                if let (Some(i1), Some(i2)) = (
                    group_to_index.get(depend),
                    group_to_index.get(&group.name),
                ) {
                    graph.add_edge(i1.1, i2.0, 1);
                }
            }
        }

        if is_cyclic_directed(&graph) {
            // TODO: Show what causes the cycle
            return Err(GraphError::Cycle);
        }

        Ok(graph)
    }
}
