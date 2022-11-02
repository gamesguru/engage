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

//! The binary

use std::{
    env,
    error::Error as StdError,
    io::{stdout, Write},
    ops::ControlFlow,
    sync::Arc,
};

use clap::Parser;
use engage::{
    args::{Args, Builtin, Just, Subcommand},
    ensure_acyclic, error, find_file, node_task_parallel, subgraph_targeting,
    Engage, Node,
};
use petgraph::{
    dot::Dot,
    graph::{DefaultIx, DiGraph},
};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match try_main(args).await {
        Ok(()) => (),
        Err(e) => {
            if let Some(error::Task::ExitStatus(e)) =
                e.downcast_ref::<error::Task>()
            {
                // Try to exit with the same status code as the failed command
                std::process::exit(e.code().unwrap_or(1));
            } else {
                // Something unusual failed, report it and error out
                eprint!("{}", error::format_cli(error::Chain(&*e)));
                std::process::exit(1);
            }
        }
    }
}

/// Fallible version of [`main`](main)
async fn try_main(args: Args) -> Result<(), Box<dyn StdError>> {
    // Find the Engage file and change the current directory to its directory
    let file = match args.file {
        None => find_file().await?,
        Some(file) => file.canonicalize()?,
    };
    env::set_current_dir(file.parent().ok_or_else(|| {
        Box::<dyn StdError>::from("path to file has no parent directory")
    })?)?;

    let contents = std::fs::read_to_string(file)?;
    let mut engage: Engage = toml::from_str(&contents)?;
    engage.update_groups();
    engage.validate()?;

    match args.subcmd {
        // Run everything
        None => {
            let graph = engage.to_graph()?;
            run_all(engage, graph).await
        }

        // Run a subgraph
        Some(Subcommand::Just(Just {
            group,
            task,
        })) => {
            let graph = subgraph_targeting(&engage.to_graph()?, group, task)?;
            run_all(engage, graph).await
        }

        // Show the Graphviz' `dot` representation of the selection of the graph
        Some(Subcommand::Builtin(Builtin::Dot {
            group,
            task,
        })) => {
            let graph = engage.to_graph()?;

            let graph = match group {
                None => graph,
                Some(group) => subgraph_targeting(&graph, group, task)?,
            };

            let x = Dot::new(&graph);

            print!("{}", x);

            // Just in case
            stdout().lock().flush()?;

            Ok(())
        }

        // List available groups and tasks
        Some(Subcommand::Builtin(Builtin::List)) => {
            // Unstable is fine because duplicate names are not allowed
            engage.groups.sort_unstable_by(|a, b| a.name.cmp(&b.name));
            engage.tasks.sort_unstable_by(|a, b| a.name.cmp(&b.name));

            for (i, group) in engage.groups.iter().enumerate() {
                println!("{}:", group.name);

                let tasks =
                    engage.tasks.iter().filter(|x| x.group == group.name);

                for task in tasks {
                    println!("    {}", task.name);
                }

                if i + 1 < engage.groups.len() {
                    println!();
                }
            }

            Ok(())
        }
    }
}

/// Run all groups and tasks in the given `Engage` object
async fn run_all(
    engage: Engage,
    graph: DiGraph<Node, u32, DefaultIx>,
) -> Result<(), Box<dyn StdError>> {
    ensure_acyclic(&graph)?;
    let engage = Arc::new(engage);

    node_task_parallel(Arc::new(graph), move |node| {
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
