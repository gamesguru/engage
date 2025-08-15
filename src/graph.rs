//! Facilities for working with the graph of groups and tasks.

use std::{
    collections::HashMap,
    fmt::Display,
    future::Future,
    ops::ControlFlow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use petgraph::{
    Direction,
    algo::tarjan_scc,
    graph::{DiGraph, IndexType},
    visit::{
        DfsEvent, Reversed, VisitMap as _, Visitable as _, depth_first_search,
    },
};
use serde::{Deserialize, Serialize};
use tokio_util::task::TaskTracker;

use crate::{
    config::{Config, Group, Task},
    error, ui,
};

/// A node in the dependency graph of tasks and groups.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub(crate) enum Node {
    /// The beginning of a group's execution.
    GroupStart(Group),

    /// A task.
    Task(Task),

    /// The end of a group's execution.
    GroupEnd(Group),
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::GroupStart(start) => write!(f, "group start: {start}"),
            Node::Task(task) => write!(f, "task: {task}"),
            Node::GroupEnd(end) => write!(f, "group end: {end}"),
        }
    }
}

/// Ensure the given graph has no cycles.
///
/// # Errors
///
/// If there are cycles, a type is returned whose [`Display`] impl explains
/// which nodes have edges that create the cycle(s).
pub(crate) fn ensure_acyclic<E, Ix>(
    graph: &DiGraph<Node, E, Ix>,
) -> Result<(), Vec<error::Cycle>>
where
    Ix: IndexType,
{
    use error::Cycle as E;

    let sccs = tarjan_scc(graph)
        .into_iter()
        .filter(|scc| {
            // Count this strongly-connected component as a cycle if there are
            // more than 1 node or if that node has a self-loop.
            scc.len() > 1
                || scc.iter().copied().fold(false, |acc, node| {
                    graph.find_edge_undirected(node, node).is_some() || acc
                })
        })
        .map(|scc| E {
            scc: scc.into_iter().map(|node| graph[node].clone()).collect(),
        })
        .collect::<Vec<_>>();

    if sccs.is_empty() {
        Ok(())
    } else {
        Err(sccs)
    }
}

/// Get a subgraph to execute only a given group or task and its dependencies.
///
/// # Errors
///
/// See [`error::NotFound`] for a list of reasons why this function can fail.
pub(crate) fn subgraph_targeting<E, Ix, S1, S2>(
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
    use error::NotFound as E;

    let group = group.as_ref();

    // Find the end node of the requested group.
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
        .ok_or_else(|| E::Group(group.to_owned()))?;

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
            .ok_or_else(|| E::Task {
                name: task.as_ref().to_owned(),
                group: group.to_owned(),
            })?,
    };

    // TODO: There's probably a better way to do this.

    let mut needed_indicies = Vec::new();

    // Go backwards to find all the dependencies.
    depth_first_search(Reversed(&graph), [target_node], |event| {
        if let DfsEvent::Discover(node_index, _) = event {
            needed_indicies.push(node_index);
        }
    });

    // Filter out irrelevant nodes (and edges).
    let subgraph = graph.filter_map(
        |i, n| needed_indicies.contains(&i).then(|| n.clone()),
        |_, e| Some(*e),
    );

    Ok(subgraph)
}

/// Run tasks in parallel based on a directed graph.
///
/// # Panics
///
/// Panics if `graph` has cycles.
pub(crate) async fn execute<N, E, Ix, F, Fut>(
    graph: &DiGraph<N, E, Ix>,
    visit: F,
) where
    N: Clone + Send + Sync + 'static,
    E: Send + Sync + 'static,
    Ix: IndexType + Send + Sync,
    F: Send + 'static + Fn(N) -> Fut,
    Fut: Future<Output = ControlFlow<()>> + Send + 'static,
{
    // If there are no nodes, there is nothing to do.
    if graph.node_count() == 0 {
        return;
    }

    let mut visit_map = graph.visit_map();
    let mut layer = graph.externals(Direction::Incoming).collect::<Vec<_>>();

    while !layer.is_empty() {
        let abort = Arc::new(AtomicBool::new(false));
        let task_tracker = TaskTracker::new();

        for &node in &layer {
            let visit = visit(graph[node].clone());
            assert!(visit_map.visit(node), "cycle detected");
            let abort = abort.clone();
            task_tracker.spawn(async move {
                if visit.await.is_break() {
                    abort.store(true, Ordering::SeqCst);
                }
            });
        }

        task_tracker.close();
        task_tracker.wait().await;

        if abort.load(Ordering::SeqCst) {
            break;
        }

        // Discover the next layer of nodes, discarding duplicate nodes due to
        // having multiple incoming edges from the previous layer.
        let mut visit_map = graph.visit_map();
        layer = layer
            .into_iter()
            .flat_map(|x| graph.neighbors(x))
            .filter(|&x| visit_map.visit(x))
            .collect();
    }
}

/// Build a graph of the groups and tasks to be executed.
///
/// # Errors
///
/// See the variants of [`error::Graph`] for why this function might fail.
pub(crate) fn build(
    config: &Config,
) -> Result<DiGraph<Node, u32>, error::Graph> {
    use error::Graph as E;

    let mut graph = DiGraph::new();

    // TODO: something more correct than this.
    let mut group_to_index = HashMap::new();
    let mut task_to_index = HashMap::new();
    let mut task_to_group = HashMap::new();

    // Add all the nodes.
    for group in config.groups.iter().cloned() {
        // Add group nodes.
        let group_start_index = graph.add_node(Node::GroupStart(group.clone()));
        let group_end_index = graph.add_node(Node::GroupEnd(group.clone()));

        group_to_index
            .insert(group.name.clone(), (group_start_index, group_end_index));

        let tasks = config.tasks.iter().filter(|t| t.group == group.name);

        // If there are no tasks, connect the group's start to its end.
        //
        // This prevents dependency cycles in groups with no tasks, which is
        // a weird edge case, but it should be prevented nonetheless.
        if tasks.clone().count() == 0 {
            graph.add_edge(group_start_index, group_end_index, 1);
        }

        // Add task nodes and an edge to its group.
        for task in tasks.clone() {
            let task_index = graph.add_node(Node::Task(task.clone()));
            graph.add_edge(group_start_index, task_index, 1);
            graph.add_edge(task_index, group_end_index, 1);
            task_to_index.insert(
                ui::names_to_prefix(&task.group, &task.name),
                task_index,
            );
            task_to_group.insert(
                (group.name.clone(), task.name.clone()),
                group.name.clone(),
            );
        }

        // Go back through the tasks to add edges for task dependencies.
        for task in tasks {
            let task_index = if let Some(x) =
                task_to_index.get(&ui::names_to_prefix(&task.group, &task.name))
            {
                *x
            } else {
                continue;
            };

            for dep in task.depends.iter().map(String::as_str) {
                let dep_index = task_to_group
                    .get(&(group.name.clone(), dep.to_owned()))
                    .filter(|g| g.as_str() == group.name.as_str())
                    .and_then(|g| {
                        task_to_index.get(&ui::names_to_prefix(g, dep))
                    });

                let dep_index = match dep_index {
                    Some(x) => *x,
                    None => {
                        return Err(E::TaskNotInGroup {
                            task: dep.to_owned(),
                            current_group: group.name.clone(),
                        });
                    }
                };

                // Require the dependency to be completed before this.
                graph.add_edge(dep_index, task_index, 1);

                // Remove redundant incoming edge to the task, if any.
                if let Some(group_start_edge) =
                    graph.find_edge(group_start_index, task_index)
                {
                    // Unless this dependency causes a self-loop.
                    if dep_index != task_index {
                        graph.remove_edge(group_start_edge);
                    }
                }

                // Remove redundant outgoing edge from the dependency, if any.
                if let Some(group_end_edge) =
                    graph.find_edge(dep_index, group_end_index)
                {
                    // Unless this dependency causes a self-loop.
                    if dep_index != task_index {
                        graph.remove_edge(group_end_edge);
                    }
                }
            }
        }
    }

    // Add the group edges, if any.
    for group in &config.groups {
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
                    return Err(E::UndefinedGroup {
                        group: group.name.clone(),
                        dependency: depend.clone(),
                    });
                }
            }
        }
    }

    Ok(graph)
}
