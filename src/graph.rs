//! Facilities for working with the graph of tasks.

use std::{
    collections::{BTreeMap, HashMap},
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
use tokio_util::task::TaskTracker;

use crate::{config::Task, error, name::Named};

/// A node in the graph of tasks.
pub(crate) type Node = Named<Task>;

/// Ensure the given graph has no cycles.
///
/// # Errors
///
/// If there are cycles, a type is returned whose [`Display`](std::fmt::Display)
/// impl explains which nodes have edges that create the cycle(s).
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

/// Get a subgraph to execute only a given task and its dependencies.
///
/// # Errors
///
/// See [`error::TaskNotFound`] for a list of reasons why this function can
/// fail.
pub(crate) fn subgraph_targeting<E, Ix, S>(
    graph: &DiGraph<Node, E, Ix>,
    task: S,
) -> Result<DiGraph<Node, E, Ix>, error::TaskNotFound>
where
    E: Copy,
    Ix: IndexType,
    S: AsRef<str>,
{
    use error::TaskNotFound as E;

    let target_node = graph
        .node_indices()
        .find(|i| {
            matches!(&graph[*i], Named {
                name,
                ..
            } if name == task.as_ref())
        })
        .ok_or_else(|| E {
            name: task.as_ref().to_owned(),
        })?;

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

/// Build a graph of the tasks to be executed.
///
/// # Errors
/// See [`error::AfterNotFound`] for why this function might fail.
pub(crate) fn build(
    tasks: &BTreeMap<String, Task>,
) -> Result<DiGraph<Node, u32>, Vec<error::AfterNotFound>> {
    use error::AfterNotFound as E;

    let mut graph = DiGraph::new();
    let mut name_to_index = HashMap::new();

    // Add nodes.
    for (name, task) in tasks {
        let index = graph.add_node(Named {
            name: name.clone(),
            value: task.clone(),
        });
        name_to_index.insert(&**name, index);
    }

    let mut errors = Vec::new();

    // Add edges.
    for (name, task) in tasks {
        for after in task.after.iter().map(String::as_str) {
            if let Some(&after) = name_to_index.get(after) {
                graph.add_edge(after, name_to_index[&**name], 1);
            } else {
                errors.push(E {
                    task: name.clone(),
                    after: after.to_owned(),
                });
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(graph)
}
