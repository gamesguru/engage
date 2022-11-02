//! Types used for working with the graph of tasks and groups thereof

use std::fmt::Display;

use petgraph::{
    algo::tarjan_scc,
    graph::{DiGraph, IndexType},
    visit::{depth_first_search, DfsEvent, Reversed},
};
use serde::{Deserialize, Serialize};

use crate::{error, Group, Task};

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
/// If there are cycles, a type whose [`Display`](std::fmt::Display) impl
/// explains which nodes have edges that create the cycles.
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
