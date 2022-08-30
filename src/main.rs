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

use std::{error::Error as StdError, sync::Arc};

use engage::{Engage, TaskError};
use petgraph::dot::Dot;

#[tokio::main]
async fn main() {
    match try_main().await {
        Ok(()) => (),
        Err(e) => println!("error: {}", engage::error::Chain(&*e)),
    }
}

/// Fallible version of [`main`](main)
async fn try_main() -> Result<(), Box<dyn StdError>> {
    let contents = std::fs::read_to_string("engage.toml")?;
    let mut engage: Engage = toml::from_str(&contents)?;
    engage.update_groups();
    let engage = Arc::new(engage);

    let graph = engage.to_graph()?;

    let x = Dot::new(&graph);

    eprint!("{}", x);

    let mut handles = Vec::with_capacity(engage.tasks.len());

    for task in engage.tasks.iter().cloned() {
        let task = Arc::new(task);
        let engage = engage.clone();
        let handle = tokio::spawn(async move { engage.run_task(task).await });

        handles.push(handle);
    }

    for handle in handles {
        let task_result = handle.await?;

        match task_result {
            Ok(()) => (),
            Err(e) => {
                if let TaskError::ExitStatus(status) = e {
                    std::process::exit(status.code().unwrap_or(1));
                } else {
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}
