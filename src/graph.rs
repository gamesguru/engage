//! Facilities for working with the graph of groups and tasks

use std::{
    collections::HashMap, fmt::Display, future::Future, ops::ControlFlow,
    sync::Arc,
};

use petgraph::{
    algo::{has_path_connecting, tarjan_scc},
    graph::{DiGraph, IndexType, NodeIndex},
    visit::{depth_first_search, DfsEvent, Reversed, VisitMap, Visitable},
    Direction,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{error, task::names_to_prefix, File, Group, Task};

/// A node in the dependency graph of tasks and groups
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Node {
    /// The beginning of a group's execution
    GroupStart(Group),

    /// A task
    Task(Task),

    /// The end of a group's execution
    GroupEnd(Group),
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::GroupStart(start) => write!(f, "group start: {}", start),
            Node::Task(task) => write!(f, "task: {}", task),
            Node::GroupEnd(end) => write!(f, "group end: {}", end),
        }
    }
}

/// Ensure the given graph has no cycles
///
/// # Errors
///
/// If there are cycles, a type is returned whose [`Display`](std::fmt::Display)
/// impl explains which nodes have edges that create the cycle(s).
pub fn ensure_acyclic<E, Ix>(
    graph: &DiGraph<Node, E, Ix>,
) -> Result<(), error::Cycle>
where
    Ix: IndexType,
{
    let sccs = tarjan_scc(graph)
        .into_iter()
        .filter(|scc| {
            // Count this strongly-connected component as a cycle if there are
            // more than 1 node or if that node has a self-loop
            scc.len() > 1
                || scc.iter().copied().fold(false, |acc, node| {
                    graph.find_edge_undirected(node, node).is_some() || acc
                })
        })
        .map(|scc| scc.into_iter().map(|node| graph[node].clone()).collect())
        .collect::<Vec<_>>();

    if sccs.is_empty() {
        Ok(())
    } else {
        Err(error::Cycle {
            sccs,
        })
    }
}

/// Get a subgraph to execute only a given group or task and its dependencies
///
/// # Errors
///
/// See [`error::NotFound`](error::NotFound) for a list of reasons why this
/// function can fail.
pub fn subgraph_targeting<E, Ix, S1, S2>(
    graph: &DiGraph<Node, E, Ix>,
    group: S1,
    task: Option<S2>,
) -> Result<DiGraph<Node, E, Ix>, error::NotFound>
where
    E: Copy,
    Ix: IndexType,
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    let group = group.as_ref();

    // Find the end node of the requested group
    //
    // Always do this even when a task is requested to produce better error
    // messages.
    let group_node = graph
        .node_indices()
        .find(|i| {
            matches!(&graph[*i], Node::GroupEnd(Group {
                name,
                ..
            }) if name == group)
        })
        .ok_or_else(|| error::NotFound::Group(group.to_owned()))?;

    let target_node = match task {
        None => group_node,
        Some(task) => graph
            .node_indices()
            .find(|i| {
                matches!(&graph[*i], Node::Task(Task {
                    name,
                    group: task_group,
                    ..
                }) if group == task_group && name == task.as_ref())
            })
            .ok_or_else(|| error::NotFound::Task {
                name: task.as_ref().to_owned(),
                group: group.to_owned(),
            })?,
    };

    // TODO: There's probably a better way to do this

    let mut needed_indicies = Vec::new();

    // Go backwards to find all the dependencies
    depth_first_search(Reversed(&graph), [target_node], |event| {
        if let DfsEvent::Discover(node_index, _) = event {
            needed_indicies.push(node_index);
        }
    });

    // Filter out irrelevant nodes (and edges)
    let subgraph = graph.filter_map(
        |i, n| needed_indicies.contains(&i).then(|| n.clone()),
        |_, e| Some(*e),
    );

    Ok(subgraph)
}

/// Run tasks in parallel based on a directed graph
///
/// This will deadlock if `graph` is not acyclic.
pub async fn execute<N, E, Ix, F, Fut, B>(
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
    // If there are no nodes, there is nothing to do
    if graph.node_count() == 0 {
        return None;
    }

    let (visit_tx, mut visit_rx) = mpsc::channel::<NodeIndex<Ix>>(16);
    let (ready_tx, mut ready_rx) = mpsc::channel(16);
    let (break_tx, mut break_rx) = mpsc::channel(1);

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

/// Get a graph of the groups and tasks to be executed
///
/// # Errors
///
/// See the variants of [`error::Graph`][error::Graph] for why this function
/// might fail.
pub fn from_file(file: &File) -> Result<DiGraph<Node, u32>, error::Graph> {
    let mut graph = DiGraph::new();

    // TODO: something more correct than this
    let mut group_to_index = HashMap::new();
    let mut task_to_index = HashMap::new();
    let mut task_to_group = HashMap::new();

    // Add all the nodes
    for group in file.groups.iter().cloned() {
        // Add group nodes
        let group_start_index = graph.add_node(Node::GroupStart(group.clone()));
        let group_end_index = graph.add_node(Node::GroupEnd(group.clone()));

        group_to_index
            .insert(group.name.clone(), (group_start_index, group_end_index));

        let tasks = file.tasks.iter().filter(|t| t.group == group.name);

        // If there are no tasks, connect the group's start to its end
        //
        // This prevents dependency cycles in groups with no tasks, which is
        // a weird edge case, but it should be prevented nonetheless.
        if tasks.clone().count() == 0 {
            graph.add_edge(group_start_index, group_end_index, 1);
        }

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
                    .and_then(|g| task_to_index.get(&names_to_prefix(g, dep)));

                let dep_index = match dep_index {
                    Some(x) => *x,
                    None => {
                        return Err(error::Graph::TaskNotInGroup {
                            task: dep.to_owned(),
                            current_group: group.name.clone(),
                        })
                    }
                };

                // Require the dependency to be completed before this
                graph.add_edge(dep_index, task_index, 1);

                // Remove redundant incoming edge to the task, if any
                if let Some(group_start_edge) =
                    graph.find_edge(group_start_index, task_index)
                {
                    // Unless this dependency causes a self-loop
                    if dep_index != task_index {
                        graph.remove_edge(group_start_edge);
                    }
                }

                // Remove redundant outgoing edge from the dependency, if
                // any
                if let Some(group_end_edge) =
                    graph.find_edge(dep_index, group_end_index)
                {
                    // Unless this dependency causes a self-loop
                    if dep_index != task_index {
                        graph.remove_edge(group_end_edge);
                    }
                }
            }
        }
    }

    // Add the group edges, if any
    for group in &file.groups {
        let group_start_index = group_to_index
            .get(&group.name)
            .map(|(start, _)| start)
            .copied()
            .expect("this should have been inserted during the first loop");

        for depend in &group.depends {
            match group_to_index.get(depend).map(|(_, end)| end).copied() {
                Some(group_end_index) => {
                    graph.add_edge(group_end_index, group_start_index, 1);
                }
                None => {
                    return Err(error::Graph::UndefinedGroup {
                        group: group.name.clone(),
                        dependency: depend.clone(),
                    })
                }
            }
        }
    }

    Ok(graph)
}
