//! Facilities for working with graphs computed from Engage files.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
    future::Future,
    ops::ControlFlow,
    sync::Arc,
};

use either::Either::{Left, Right};
use futures_concurrency::future::FutureExt as _;
use futures_util::{FutureExt as _, pin_mut};
use petgraph::{
    Direction,
    algo::tarjan_scc,
    graph::{DiGraph, IndexType, NodeIndex},
    visit::{
        DfsEvent, Reversed, VisitMap as _, Visitable as _, depth_first_search,
    },
};
use tokio::sync::mpsc;
use tokio_stream::{
    StreamExt as _,
    wrappers::{ReceiverStream, UnboundedReceiverStream},
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    config::{Process, ReadyWhen},
    error,
    name::{Name, Named},
};

/// A graph of processes.
pub(crate) type ProcessGraph = DiGraph<Arc<Named<Process>>, EdgeKind>;

/// The kind of an edge in the graph.
#[derive(Debug, Copy, Clone)]
pub(crate) enum EdgeKind {
    /// The edge is created by a `before` dependency.
    Before,

    /// The edge is created by an `after` dependency.
    After,
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdgeKind::Before => write!(f, "via 'before'"),
            EdgeKind::After => write!(f, "via 'after'"),
        }
    }
}

/// Get a subgraph of the given processes and their dependencies.
pub(crate) fn subgraph(
    graph: &ProcessGraph,
    processes: &BTreeSet<Box<Name>>,
) -> Result<ProcessGraph, BTreeSet<error::ProcessNotFound>> {
    use error::ProcessNotFound as E;

    let mut found = HashMap::new();
    for ix in graph.node_indices() {
        let name = &*graph[ix].name;

        if processes.contains(name) {
            found.insert(name, ix);
        }
    }

    let mut errs = BTreeSet::new();

    for name in processes {
        if !found.contains_key(&**name) {
            errs.insert(E(name.clone()));
        }
    }

    if !errs.is_empty() {
        return Err(errs);
    }

    // TODO: There's probably a better way to do this.

    let mut needed_indicies = Vec::new();

    // Go backwards to find all the dependencies.
    depth_first_search(Reversed(&graph), found.values().copied(), |event| {
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

/// Run `visit` for each node in `graph` in parallel, ordered by `graph`'s
/// edges.
///
/// Visiting starts at the nodes with no `direction` edges and follows edges
/// in the opposite direction of `direction` to find the next nodes to visit.
///
/// Traversal will be cancelled early if `cancelled` completes or if any `visit`
/// call returns [`ControlFlow::Break`]. Any `visit` calls that have been
/// started will still be polled to completion before this function returns.
pub(crate) async fn edge_order_par_visit<C, N, E, Ix, F, Fut>(
    cancelled: C,
    graph: &DiGraph<N, E, Ix>,
    direction: Direction,
    visit: F,
) where
    C: Future<Output = ()>,
    N: Send + Sync + 'static,
    E: Send + Sync + 'static,
    Ix: IndexType + Send + Sync,
    F: Send + 'static + Fn(NodeIndex<Ix>) -> Fut,
    Fut: Future<Output = ControlFlow<()>> + Send + 'static,
{
    let mut visit_map = graph.visit_map();

    // If there are no nodes, there is nothing to do.
    if visit_map.is_full() {
        return;
    }

    pin_mut!(cancelled);
    let loop_ct = CancellationToken::new();
    let task_tracker = TaskTracker::new();
    let (visited_tx, visited_rx) = mpsc::channel(1);

    // Use an unbounded channel to avoid deadlocks because:
    //
    // * Multiple indices can become ready to visit at once.
    // * The producer and consumer are multiplexed on a single task.
    let (visit_tx, visit_rx) = mpsc::unbounded_channel();

    let mut ixes = tokio_stream::iter(graph.externals(direction))
        .chain(UnboundedReceiverStream::new(visit_rx))
        .map(Left)
        .merge(ReceiverStream::new(visited_rx).map(Right));

    while let Some(ix) = ixes
        .next()
        .race(loop_ct.cancelled().map(|()| None))
        .race(cancelled.as_mut().map(|()| None))
        .await
    {
        match ix {
            // Visit an index.
            Left(ix) => {
                let visit = visit(ix);
                let loop_ct = loop_ct.clone();
                let visited_tx = visited_tx.clone();

                task_tracker.spawn(async move {
                    if visit.await.is_break() {
                        loop_ct.cancel();
                    } else {
                        // Race with loop_ct to avoid a deadlock between sending
                        // on visited_tx and waiting on task_tracker because
                        // visited_rx will no longer be read from at that point.
                        visited_tx
                            .send(ix)
                            .map(|x| x.expect("channel should be open"))
                            .race(loop_ct.cancelled())
                            .await;
                    }
                });
            }

            // Compute new indices to visit.
            Right(ix) => {
                visit_map.visit(ix);

                if visit_map.is_full() {
                    break;
                }

                let ixes = graph
                    .neighbors_directed(ix, direction.opposite())
                    .filter(|&ix| {
                        graph
                            .neighbors_directed(ix, direction)
                            .all(|ix| visit_map.contains(ix.index()))
                    });

                for ix in ixes {
                    visit_tx.send(ix).expect("channel should be open");
                }
            }
        }
    }

    task_tracker.close();
    task_tracker.wait().await;
}

/// Build a graph that can be run.
pub(crate) fn build(
    processes: &BTreeMap<Box<Name>, Process>,
) -> (ProcessGraph, Vec<error::BuildGraph>) {
    use error::BuildGraph as E;

    let mut errors = Vec::new();
    let mut graph = ProcessGraph::new();
    let mut name_to_index = HashMap::new();
    let mut name_to_parts = HashMap::<_, Vec<_>>::new();

    for (p_name, p_config) in processes.iter().map(|(k, v)| (&**k, v)) {
        // Insert a node for each process into the graph.
        let index = graph.add_node(Arc::new(Named {
            name: p_name.to_owned(),
            value: p_config.clone(),
        }));

        // Build the lookup table from process names to their node index.
        name_to_index.insert(p_name, index);

        let Some(po_name) = p_config.part_of.as_deref() else {
            continue;
        };

        let Some(po_config) = processes.get(po_name) else {
            errors.push(E::PartOfNotFound {
                process: p_name.to_owned(),
                part_of: po_name.to_owned(),
            });
            continue;
        };

        if p_config.ready_when == ReadyWhen::Spawned
            && po_config.ready_when == ReadyWhen::Exited
        {
            errors.push(E::ServicePartOfTask {
                process: p_name.to_owned(),
                part_of: po_name.to_owned(),
            });

            // Don't abort this iteration so that the resulting edges are
            // included in the graph despite this error.
        }

        // Build the lookup table from a process name to the names of the
        // processes that are part of it. Processes are not considered part of
        // themselves. Processes without any parts will not have a corresponding
        // key in the table.
        name_to_parts.entry(po_name).or_default().push(p_name);
    }

    // Insert edges between processes into the graph.
    for (p_name, p_config) in processes.iter().map(|(k, v)| (&**k, v)) {
        for (d_names, edge_kind) in [
            (&p_config.after, EdgeKind::After),
            (&p_config.before, EdgeKind::Before),
        ] {
            let edge_order_indices = |p_name, d_name| match edge_kind {
                EdgeKind::Before => {
                    (name_to_index[p_name], name_to_index[d_name])
                }
                EdgeKind::After => {
                    (name_to_index[d_name], name_to_index[p_name])
                }
            };

            for d_name in d_names.iter().map(|x| &**x) {
                // Reject and omit the edges if the dependency doesn't exist.
                let Some(d_config) = processes.get(d_name) else {
                    errors.push(E::DependencyNotFound {
                        process: p_name.to_owned(),
                        dependency: d_name.to_owned(),
                        edge_kind,
                    });
                    continue;
                };

                // Reject but include the edges if the dependency is not part of
                // the same process as this process and this process is part of
                // any process.
                if let Some(part_of) = p_config.part_of.as_deref()
                    && d_config.part_of.as_deref().is_none_or(|x| x != part_of)
                    && d_name != part_of
                {
                    errors.push(E::DependencyNotPartOf {
                        process: p_name.to_owned(),
                        process_part_of: part_of.to_owned(),
                        dependency: d_name.to_owned(),
                        dependency_part_of: d_config.part_of.clone(),
                        edge_kind,
                    });
                }

                // Reject but include the edges if the process is not part of
                // the same process as this dependency and this dependency is
                // part of any process.
                if let Some(part_of) = d_config.part_of.as_deref()
                    && p_config.part_of.as_deref().is_none_or(|x| x != part_of)
                    && p_name != part_of
                {
                    errors.push(E::ProcessNotPartOf {
                        process: p_name.to_owned(),
                        process_part_of: p_config.part_of.clone(),
                        dependency: d_name.to_owned(),
                        dependency_part_of: part_of.to_owned(),
                        edge_kind,
                    });
                }

                // Add implicit edges if the dependency is comprised of multiple
                // parts and the current process is not part of any other
                // process. That second condition is necessary to avoid creating
                // self-loops.
                if let Some(parts) = name_to_parts.get(d_name).map(|x| &**x)
                    && p_config.part_of.is_none()
                {
                    for &part in parts {
                        let (source, target) = edge_order_indices(p_name, part);
                        graph.add_edge(source, target, edge_kind);
                    }
                }

                // Add the explicit edge.
                let (source, target) = edge_order_indices(p_name, d_name);
                graph.add_edge(source, target, edge_kind);
            }
        }
    }

    find_cycles(&graph, &mut errors);

    (graph, errors)
}

/// Find cycles in the graph.
fn find_cycles(graph: &ProcessGraph, errors: &mut Vec<error::BuildGraph>) {
    use error::BuildGraph as E;

    let iter = tarjan_scc(graph)
        .into_iter()
        .filter(|scc| {
            // Count this strongly-connected component as a cycle if there are
            // more than 1 node or if that node has a self-loop.
            scc.len() > 1
                || scc.iter().copied().fold(false, |acc, node| {
                    graph.find_edge_undirected(node, node).is_some() || acc
                })
        })
        .map(|scc| {
            E::DependencyCycle(
                scc.into_iter().map(|node| graph[node].clone()).collect(),
            )
        });

    errors.extend(iter);
}
