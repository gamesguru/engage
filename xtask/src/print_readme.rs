//! Prints the new readme contents to `stdout`.

use std::{fs, io::Write};

use far::{find, Render};
use textwrap::{fill, Options};

/// Replacements to make in `README.template.md`
#[derive(Render)]
struct Replacements {
    /// Contents of `tagline.txt`
    tagline: String,

    /// Contents of `example.toml`
    example_toml: String,

    /// Contents of `behavior.md`
    ///
    /// Make sure to reflow this so `markdownlint` likes it.
    behavior: String,
}

/// xtask entrypoint
pub(crate) fn main() -> Result<(), Box<dyn std::error::Error>> {
    let readme_template_md = fs::read_to_string("assets/README.template.md")?;
    let tagline = fs::read_to_string("assets/tagline.txt")?;
    let example_toml = fs::read_to_string("assets/example.toml")?;
    let behavior_md = fs::read_to_string("assets/behavior.md")?;

    let opts = Options::new(80).subsequent_indent("  ");

    let behavior_md =
        behavior_md.lines().fold(String::new(), |mut acc, line| {
            acc.push_str(&fill(line, &opts));
            acc.push('\n');
            acc
        });

    let found = find(readme_template_md)?;

    let replacements = Replacements {
        tagline: tagline.trim_end_matches('\n').to_owned(),
        example_toml: example_toml.trim_end_matches('\n').to_owned(),
        behavior: behavior_md.trim_end_matches('\n').to_owned(),
    };

    let s = found.replace(&replacements);

    print!("{s}");
    std::io::stdout().flush()?;

    Ok(())
}
