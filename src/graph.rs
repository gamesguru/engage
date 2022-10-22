//! Types used for working with the graph of tasks and groups thereof

use std::fmt::{self, Display};

use petgraph::{
    algo::tarjan_scc,
    graph::{DiGraph, IndexType},
};
use serde::{Deserialize, Serialize};

use crate::{task::names_to_prefix, Group, Task};

/// Errors that can occur when producing a DAG of groups and tasks
#[derive(thiserror::Error, Debug)]
pub enum Error {
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
}

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

/// The graph of groups and tasks is not acyclic
#[derive(Debug, thiserror::Error)]
pub struct CycleError {
    /// A list of pre-formatted strongly connected components
    sccs: Vec<Vec<Node>>,
}

impl Display for CycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_string = |node: &Node| match node {
            Node::Task(x) => names_to_prefix(&x.group, &x.name),
            Node::GroupStart(_) | Node::GroupEnd(_) => node.to_string(),
        };

        write!(
            f,
            "a dependency cycle is created by the edges between the node \
             set{} ",
            if self.sccs.len() == 1 {
                ""
            } else {
                "s"
            }
        )?;

        for (is_last, scc) in self
            .sccs
            .iter()
            .enumerate()
            .map(|(i, x)| (i + 1 == self.sccs.len(), x))
        {
            let at_least_two = scc.len() >= 2;
            let exactly_two = scc.len() == 2;

            for (is_last, node) in
                scc.iter().enumerate().map(|(i, x)| (i + 1 == scc.len(), x))
            {
                if is_last && at_least_two {
                    write!(f, r#"and "{}""#, to_string(node))?;
                } else if is_last {
                    write!(f, r#""{}""#, to_string(node))?;
                } else if exactly_two {
                    write!(f, r#""{}" "#, to_string(node))?;
                } else {
                    write!(f, r#""{}", "#, to_string(node))?;
                }
            }
            if !is_last {
                write!(f, "; ")?;
            }
        }

        Ok(())
    }
}

/// Ensure the given graph has no cycles
///
/// # Errors
///
/// If there are cycles, a type whose [`Display`](fmt::Display) impl explains
/// which nodes have edges that create the cycles.
pub fn ensure_acyclic<E, Ix>(
    graph: &DiGraph<Node, E, Ix>,
) -> Result<(), CycleError>
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
        Err(CycleError {
            sccs,
        })
    }
}
