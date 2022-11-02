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

//! Internal functionality for the binary crate of the same name

use std::{env, io, path::PathBuf};

use tokio::fs;

pub use crate::{
    engage::Engage,
    task::{Group, Task},
};

pub mod args;
mod engage;
pub mod error;
pub mod graph;
mod task;

/// Defines the `FILE_NAME` static
macro_rules! define_file_name {
    ($name:literal) => {
        #[doc = "The canonical name of the Engage file: `"]
        #[doc = $name]
        #[doc = "`\n"]
        /// # Why that name?
        ///
        /// After reading through [this issue][issue] and [this internals
        /// discussion][discussion], the only thing I could decide for sure was
        /// that there should be exactly one allowed form, for the sake of
        /// consistency across projects.
        ///
        /// I'm okay with both the all-lowercase and first-char-uppercase
        /// conventions, because the former is consistent with pretty much
        /// everything else on sane systems, and the latter stands out, making
        /// it easy to spot, so you know a project uses the tool in question.
        ///
        /// After much indecision and talking with other people about it, a
        /// friend recommended I flip a coin. So I did, and all-lowercase was
        /// chosen first, and won best 2 out of 3, and won best 3 out of 5, in
        /// the same coin-flipping session. So, all-lowercase it is.
        ///
        /// [issue]: https://github.com/rust-lang/cargo/issues/45
        /// [discussion]: https://internals.rust-lang.org/t/can-we-rename-cargo-toml/380
        pub static FILE_NAME: &str = $name;
    };
}

define_file_name!("engage.toml");

/// The separator that appears between the task's name and group and its output
pub static OUTPUT_SEPARATOR: &str = "│";

/// The separator between the task group and name
pub static TASK_GROUP_NAME_SEPARATOR: &str = "::";

/// Things that cannot be used as group names
pub static ILLEGAL_GROUP_NAMES: &[&str] = &["self", "just", "help"];

/// Search upwards until an Engage file is found, returning the path to it
///
/// Does not change the current directory of the calling process, that must be
/// done manually if desired.
#[allow(clippy::missing_errors_doc)]
pub async fn find_file() -> io::Result<PathBuf> {
    let mut search_dir = env::current_dir()?;

    loop {
        let mut read_dir = fs::read_dir(&search_dir).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            if entry.file_name() == FILE_NAME {
                return Ok(entry.path());
            }
        }

        if !search_dir.pop() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "{FILE_NAME} not found in the current directory or its \
                     ancestors"
                ),
            ));
        }
    }
}
