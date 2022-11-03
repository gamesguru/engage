//! Things to do with the "user interface" of the command line tool

use std::fmt;

use crossterm::{
    execute,
    style::{Print, Stylize},
};

/// The separator that appears between the task's name and group and its output
pub static OUTPUT_SEPARATOR: &str = "│";

/// The separator between the task group and name
pub static TASK_GROUP_NAME_SEPARATOR: &str = "::";

/// Formats an error message to be printed on the command line
///
/// The returned string includes a trailing newline.
#[must_use]
pub fn format_error<D>(error: D) -> String
where
    D: fmt::Display,
{
    let mut buf = Vec::new();

    execute!(
        buf,
        Print("error".red().bold()),
        Print(':'.bold()),
        Print(' '),
        Print(error),
        Print('\n'),
    )
    .expect("should be able to write to in-memory buffer");

    String::from_utf8(buf).expect("should be a valid UTF-8 string")
}

/// Get a unique combination of group and task names
pub(crate) fn names_to_prefix<S1, S2>(group: S1, task: S2) -> String
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    format!("{}{}{}", group.as_ref(), TASK_GROUP_NAME_SEPARATOR, task.as_ref())
}
