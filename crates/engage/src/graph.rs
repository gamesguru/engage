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
    config::Process,
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

    for (p_name, p_config) in processes.iter().map(|(k, v)| (&**k, v)) {
        // Insert a node for each process into the graph.
        let index = graph.add_node(Arc::new(Named {
            name: p_name.to_owned(),
            value: p_config.clone(),
        }));

        // Build the lookup table from process names to their node index.
        name_to_index.insert(p_name, index);
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
                if !name_to_index.contains_key(d_name) {
                    errors.push(E::DependencyNotFound {
                        process: p_name.to_owned(),
                        dependency: d_name.to_owned(),
                        edge_kind,
                    });
                    continue;
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
