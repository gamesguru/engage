//! Facilities for working with graphs computed from Engage files.

use std::{
    collections::{BTreeMap, HashMap},
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

/// The kind of an edge in the graph.
#[derive(Copy, Clone)]
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

/// Ensure the given graph has no cycles.
///
/// # Errors
///
/// If there are cycles, a type is returned whose [`Display`](std::fmt::Display)
/// impl explains which nodes have edges that create the cycle(s).
pub(crate) fn ensure_acyclic<E, Ix>(
    graph: &DiGraph<Arc<Named<Process>>, E, Ix>,
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

/// Get a subgraph to run only a given process and its dependencies.
///
/// # Errors
///
/// See [`error::ProcessNotFound`] for a list of reasons why this function can
/// fail.
pub(crate) fn subgraph_targeting<E, Ix, S>(
    graph: &DiGraph<Arc<Named<Process>>, E, Ix>,
    process: S,
) -> Result<DiGraph<Arc<Named<Process>>, E, Ix>, error::ProcessNotFound>
where
    E: Copy,
    Ix: IndexType,
    S: AsRef<Name>,
{
    use error::ProcessNotFound as E;

    let target_node = graph
        .node_indices()
        .find(|i| {
            matches!(&*graph[*i], Named {
                name,
                ..
            } if *name == process.as_ref())
        })
        .ok_or_else(|| E {
            name: process.as_ref().to_owned(),
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
///
/// # Errors
///
/// See [`error::BuildGraph`] for why this function might fail.
pub(crate) fn build(
    processes: &BTreeMap<Box<Name>, Process>,
) -> Result<DiGraph<Arc<Named<Process>>, EdgeKind>, Vec<error::BuildGraph>> {
    use error::BuildGraph as E;

    let mut graph = DiGraph::new();
    let mut name_to_index = HashMap::new();

    // Add nodes.
    for (name, process) in processes {
        let index = graph.add_node(Arc::new(Named {
            name: name.clone(),
            value: process.clone(),
        }));
        name_to_index.insert(&**name, index);
    }

    let mut errors = Vec::new();

    // Add edges.
    for (name, process) in processes.iter().map(|(n, t)| (&**n, t)) {
        for after in process.after.iter().map(|x| &**x) {
            if let Some(&after) = name_to_index.get(after) {
                graph.add_edge(after, name_to_index[name], EdgeKind::After);
            } else {
                errors.push(E::AfterNotFound {
                    process: name.to_owned(),
                    after: after.to_owned(),
                });
            }
        }

        for before in process.before.iter().map(|x| &**x) {
            if let Some(&before) = name_to_index.get(before) {
                graph.add_edge(name_to_index[name], before, EdgeKind::Before);
            } else {
                errors.push(E::BeforeNotFound {
                    process: name.to_owned(),
                    before: before.to_owned(),
                });
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(graph)
}
