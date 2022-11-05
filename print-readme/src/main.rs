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

//! Small program to update the readme
//!
//! Prints the new contents to `stdout`.

use std::{fs, io::Write};

use far::{find, Render};
use textwrap::{fill, Options};

/// Replacements to make in `README.template.md`
#[derive(Render)]
struct Replacements {
    /// Contents of `example.toml`
    example_toml: String,

    /// Contents of `behavior.md`
    ///
    /// Make sure to reflow this so `markdownlint` likes it.
    behavior: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let readme_template_md = fs::read_to_string("assets/README.template.md")?;
    let example_toml = fs::read_to_string("assets/example.toml")?;
    let behavior_md = fs::read_to_string("assets/behavior.md")?;

    let opts = Options::new(80).subsequent_indent("  ");

    let behavior_md =
        behavior_md.lines().fold(String::new(), |mut acc, line| {
            acc.push_str(&fill(line, &opts));
            acc.push('\n');
            acc
        });

    let found = find(&readme_template_md)?;

    let replacements = Replacements {
        example_toml: example_toml.trim_end_matches('\n').to_owned(),
        behavior: behavior_md.trim_end_matches('\n').to_owned(),
    };

    let s = found.replace(&replacements);

    print!("{}", s);
    std::io::stdout().flush()?;

    Ok(())
}
