//! Prints the new readme contents to `stdout`.

use std::{fs, io::Write};

use far::{find, Render};

/// Replacements to make in `README.template.md`
#[derive(Render)]
struct Replacements {
    /// Contents of `tagline.txt`
    tagline: String,
}

/// xtask entrypoint
pub(crate) fn main() -> Result<(), Box<dyn std::error::Error>> {
    let readme_template_md = fs::read_to_string("assets/README.template.md")?;
    let tagline = fs::read_to_string("assets/tagline.txt")?;

    let found = find(readme_template_md)?;

    let replacements = Replacements {
        tagline: tagline.trim_end_matches('\n').to_owned(),
    };

    let s = found.replace(&replacements);

    print!("{s}");
    std::io::stdout().flush()?;

    Ok(())
}
