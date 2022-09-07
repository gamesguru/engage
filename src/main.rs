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

use std::{error::Error as StdError, ops::ControlFlow, sync::Arc};

use engage::{node_task_parallel, Engage, Node, TaskError};
use petgraph::dot::Dot;

#[tokio::main]
async fn main() {
    match try_main().await {
        Ok(()) => (),
        Err(e) => {
            if let Some(TaskError::ExitStatus(e)) =
                e.downcast_ref::<TaskError>()
            {
                // Try to exit with the same status code as the failed command
                std::process::exit(e.code().unwrap_or(1));
            } else {
                // Something unusual failed, report it and error out
                println!("error: {}", engage::error::Chain(&*e));
                std::process::exit(1);
            }
        }
    }
}

/// Fallible version of [`main`](main)
async fn try_main() -> Result<(), Box<dyn StdError>> {
    let contents = std::fs::read_to_string("engage.toml")?;
    let mut engage: Engage = toml::from_str(&contents)?;
    engage.update_groups();
    let engage = Arc::new(engage);

    let graph = engage.to_graph()?;
    let graph = Arc::new(graph);

    let x = Dot::new(graph.as_ref());

    eprint!("{}", x);

    node_task_parallel(graph.clone(), move |node| {
        let engage = engage.clone();
        async move {
            if let Node::Task(task) = node {
                if let Err(e) = engage.run_task(Arc::new(task)).await {
                    return ControlFlow::Break(e);
                }
            }

            ControlFlow::Continue(())
        }
    })
    .await
    .map_or_else(|| Ok(()), |e| Err(e.into()))
}
