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
    collections::HashMap,
    fmt::Display,
    future::Future,
    io::{self, stdout},
    ops::ControlFlow,
    process::{ExitStatus, Stdio},
    sync::Arc,
};

use crossterm::{
    execute,
    style::{Print, Stylize},
};
use petgraph::{
    algo::{has_path_connecting, is_cyclic_directed},
    graph::{IndexType, NodeIndex},
    prelude::DiGraph,
    visit::{VisitMap, Visitable},
    Direction,
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    sync::mpsc::channel,
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
    #[serde(default, rename = "task")]
    pub tasks: Vec<Task>,

    /// Configuration of task groups
    #[serde(default, rename = "group")]
    pub groups: Vec<Group>,
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

    /// Other tasks this task depends on
    ///
    /// Tasks must be within the same group.
    #[serde(default)]
    pub depends: Vec<String>,
}

/// A task group within `engage.toml`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Group {
    /// Name of the group of tasks
    pub name: String,

    /// List of groups that need to run before this one
    #[serde(default)]
    pub depends: Vec<String>,
}

/// A node in the dependency graph of tasks and groups
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Node {
    /// The beginning of a group's execution
    GroupStart(Group),

    /// A task
    Task(Task),

    /// The end of a group's execution
    GroupEnd,
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::GroupStart(start) => write!(f, "group start: {}", start),
            Node::Task(task) => write!(f, "task: {}", task),
            Node::GroupEnd => write!(f, "group end"),
        }
    }
}

impl Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Engage {
    /// Returns the length of the longest prefix
    #[must_use]
    pub fn longest_prefix(&self) -> usize {
        let mut longest = 0;
        for task in &self.tasks {
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

/// Errors that can occur when producing a DAG of groups and tasks
#[derive(thiserror::Error, Debug)]
pub enum GraphError {
    /// A task dependends on another task that belongs to a different group
    #[error(
        "dependency task \"{task}\" does not belong to group \
         \"{current_group}\""
    )]
    TaskNotInGroup {
        /// The task being depended upon
        task: String,

        /// The group the current task belongs to
        current_group: String,
    },

    /// Groups and tasks were not acyclic
    #[error("dependency cycle detected")]
    Cycle,
}

impl Task {
    /// Get the log prefix of this task
    #[must_use]
    pub fn to_prefix(&self) -> String {
        names_to_prefix(&self.group, &self.name)
    }
}

/// Get a unique combination of group and task names
fn names_to_prefix<S1, S2>(group: S1, task: S2) -> String
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    format!("{}{}{}", group.as_ref(), PREFIX_SEPARATOR, task.as_ref())
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

/// Run tasks in parallel based on a directed graph
///
/// This will deadlock if `graph` is not acyclic.
// TODO: allow executing a subgraph?
pub async fn node_task_parallel<N, E, Ix, F, Fut, B>(
    graph: Arc<DiGraph<N, E, Ix>>,
    task: F,
) -> Option<B>
where
    N: Clone + Send + Sync + 'static,
    E: Send + Sync + 'static,
    Ix: IndexType + Send + Sync,
    F: Send + 'static + Fn(N) -> Fut,
    Fut: Future<Output = ControlFlow<B>> + Send + 'static,
    B: std::fmt::Debug + Send + 'static,
{
    let (visit_tx, mut visit_rx) = channel::<NodeIndex<Ix>>(16);
    let (ready_tx, mut ready_rx) = channel(16);
    let (break_tx, mut break_rx) = channel(1);

    // A background task that consumes newly-visited nodes and produces
    // newly-readied nodes
    let _scheduler = {
        let mut visit_map = graph.visit_map();
        let graph = graph.clone();
        tokio::spawn(async move {
            while let Some(visited) = visit_rx.recv().await {
                visit_map.visit(visited);

                let ready_nodes = graph
                    .node_indices()
                    .filter(|node| {
                        // We only care about nodes connected to this
                        // visited node
                        has_path_connecting(
                            graph.as_ref(),
                            visited,
                            *node,
                            None,
                        )
                    })
                    .filter(|node| {
                        // We only care about this node's dependency
                        graph
                            .neighbors_directed(*node, Direction::Incoming)
                            .all(|node| visit_map.is_visited(&node))
                    })
                    .filter(|node| {
                        // We don't want to revisit nodes
                        !visit_map.is_visited(node)
                    });

                for node in ready_nodes {
                    ready_tx.send(node).await.expect("channel closed");
                }

                let all_nodes_visited = graph
                    .node_indices()
                    .all(|node| visit_map.is_visited(&node));

                if all_nodes_visited {
                    // We're done!
                    return;
                }
            }
        })
    };

    // Execute the initial nodes and any nodes that become ready afterward
    let _executor = tokio::spawn(async move {
        let nodes_to_execute = graph.externals(Direction::Incoming);

        // Execute the initial nodes
        for node in nodes_to_execute {
            let task = task(graph[node].clone());

            let visit_tx = visit_tx.clone();
            let break_tx = break_tx.clone();
            tokio::spawn(async move {
                match task.await {
                    ControlFlow::Continue(()) => {
                        visit_tx.send(node).await.expect("channel closed");
                    }
                    ControlFlow::Break(b) => {
                        break_tx.send(b).await.expect("channel closed");
                    }
                }
            });
        }

        // Execute all nodes that become ready as a result of the initial nodes
        // being visited
        while let Some(node) = ready_rx.recv().await {
            let task = task(graph[node].clone());

            {
                let visit_tx = visit_tx.clone();
                let break_tx = break_tx.clone();
                tokio::spawn(async move {
                    match task.await {
                        ControlFlow::Continue(()) => {
                            visit_tx.send(node).await.expect("channel closed");
                        }
                        ControlFlow::Break(b) => {
                            break_tx.send(b).await.expect("channel closed");
                        }
                    }
                });
            }
        }
    });

    // Will either give some break value or the senders will all be dropped when
    // all nodes are executed normally
    break_rx.recv().await
}
