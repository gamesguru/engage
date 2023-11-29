//! Facilities for working with the graph of groups and tasks

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
    future::Future,
    ops::ControlFlow,
    sync::Arc,
};

use petgraph::{
    algo::{has_path_connecting, tarjan_scc},
    graph::{DiGraph, IndexType, NodeIndex},
    visit::{depth_first_search, DfsEvent, Reversed, VisitMap, Visitable},
    Directed, Direction, Graph,
};
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinSet, time::Instant};

use crate::{error, file, ui};

/// A node in the dependency graph of tasks and groups
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub(crate) enum Node {
    /// The beginning of a group's execution
    GroupStart(file::Group),

    /// A task
    Task(file::Task),

    /// The end of a group's execution
    GroupEnd(file::Group),
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

/// Ensure the given graph has no cycles
///
/// # Errors
///
/// If there are cycles, a type is returned whose [`Display`] impl explains
/// which nodes have edges that create the cycle(s).
pub(crate) fn ensure_acyclic<E, Ix>(
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
    let group = group.as_ref();

    // Find the end node of the requested group
    //
    // Always do this even when a task is requested to produce better error
    // messages.
    let group_node = graph
        .node_indices()
        .find(|i| {
            matches!(&graph[*i], Node::GroupEnd(file::Group {
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
                matches!(&graph[*i], Node::Task(file::Task {
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
pub(crate) async fn execute<N, E, Ix, F, Fut, B>(
    graph: Arc<DiGraph<N, E, Ix>>,
    task: F,
) -> BTreeMap<Instant, B>
where
    N: Clone + Send + Sync + 'static,
    E: Send + Sync + 'static,
    Ix: IndexType + Send + Sync,
    F: Send + 'static + Fn(N) -> Fut,
    Fut: Future<Output = ControlFlow<B>> + Send + 'static,
    B: std::fmt::Debug + Send + Sync + 'static,
{
    // If there are no nodes, there is nothing to do
    if graph.node_count() == 0 {
        return BTreeMap::new();
    }

    let (visit_tx, visit_rx) = mpsc::channel(16);
    let (ready_tx, ready_rx) = mpsc::channel(16);

    let scheduler = tokio::spawn(scheduler(graph.clone(), visit_rx, ready_tx));

    let scheduler_handle = Arc::new(scheduler.abort_handle());

    let executor = tokio::spawn(executor(
        graph,
        task,
        ready_rx,
        visit_tx,
        scheduler_handle,
    ));

    scheduler.await.expect("should be able to join scheduler");
    executor.await.expect("should be able to join executor")
}

/// Consumes visited nodes and produces readied nodes based on the graph
async fn scheduler<N, E, Ix>(
    graph: Arc<Graph<N, E, Directed, Ix>>,
    mut visit_rx: mpsc::Receiver<(NodeIndex<Ix>, bool)>,
    ready_tx: mpsc::Sender<NodeIndex<Ix>>,
) where
    Ix: IndexType,
{
    let mut visit_map = graph.visit_map();

    for entrypoint in graph.externals(Direction::Incoming) {
        ready_tx.send(entrypoint).await.expect("channel should still be open");
    }

    while let Some((visited, ok)) = visit_rx.recv().await {
        visit_map.visit(visited);

        let all_nodes_visited =
            graph.node_indices().all(|node| visit_map.is_visited(&node));

        if all_nodes_visited || !ok {
            // We're done!
            return;
        }

        let ready_nodes = graph
            .node_indices()
            .filter(|node| {
                // We only care about nodes connected to this
                // visited node
                has_path_connecting(graph.as_ref(), visited, *node, None)
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
            ready_tx.send(node).await.expect("channel should still be open");
        }
    }
}

/// Consumes and executes readied nodes and produces visited nodes
async fn executor<N, E, Ix, F, Fut, B>(
    graph: Arc<Graph<N, E, Directed, Ix>>,
    task: F,
    mut ready_rx: mpsc::Receiver<NodeIndex<Ix>>,
    visit_tx: mpsc::Sender<(NodeIndex<Ix>, bool)>,
    scheduler_handle: Arc<tokio::task::AbortHandle>,
) -> BTreeMap<Instant, B>
where
    N: Clone,
    Ix: IndexType + Send,
    F: Fn(N) -> Fut,
    Fut: Future<Output = ControlFlow<B>> + Send + 'static,
    B: std::fmt::Debug + Send + Sync + 'static,
{
    let mut join_set = JoinSet::new();

    while let Some(node) = ready_rx.recv().await {
        let task = task(graph[node].clone());

        let visit_tx = visit_tx.clone();
        let scheduler_handle = scheduler_handle.clone();
        join_set.spawn(async move {
            let control_flow = task.await;

            let result =
                visit_tx.send((node, control_flow.is_continue())).await;

            // `is_finished()` has a pretty big caveat so I wouldn't be too
            // surprised if this results in spurious panics. Hopefully this
            // works how I want, and worst-case we can just always ignore
            // a send error, which *should* be fine.
            if !scheduler_handle.is_finished() {
                // This is only a problem if the scheduler is still alive
                result.expect("channel should still be open");
            }

            (control_flow, Instant::now())
        });
    }

    let mut results = BTreeMap::new();

    // Join all tasks
    while let Some(result) = join_set.join_next().await {
        if let (ControlFlow::Break(x), exit_instant) =
            result.expect("should be able to join task")
        {
            results.insert(exit_instant, x);
        }
    }

    results
}

/// Get a graph of the groups and tasks to be executed
///
/// # Errors
///
/// See the variants of [`error::Graph`] for why this function might fail.
pub(crate) fn from_file(
    file: &file::File,
) -> Result<DiGraph<Node, u32>, error::Graph> {
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
            task_to_index.insert(
                ui::names_to_prefix(&task.group, &task.name),
                task_index,
            );
            task_to_group.insert(
                (group.name.clone(), task.name.clone()),
                group.name.clone(),
            );
        }

        // Go back through the tasks to add edges for task dependencies
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
