# Keep sorted.
{
  cargo-insta,
  cargo-llvm-cov,
  dejavu_fonts,
  graphviz,
  makeFontsConf,
  markdownlint-cli,
  mdbook,
  mkShell,
  pkgs,
  stdenv,
  toolchain,
}:

(mkShell.override { inherit stdenv; }) {
  env = {
    # Rust Analyzer needs to be able to find the path to default crate sources,
    # and it can read this environment variable to do so. The `rust-src`
    # component is required in order for this to work.
    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";

    # Give GraphViz access to the same fonts locally and in CI.
    FONTCONFIG_FILE = makeFontsConf {
      fontDirectories = [
        dejavu_fonts
      ];
    };
  };

  # Keep sorted.
  packages = [
    cargo-insta
    cargo-llvm-cov
    graphviz
    markdownlint-cli
    mdbook
    toolchain
  ]
  # Keep sorted.
  ++ pkgs.default.buildInputs
  ++ pkgs.default.nativeBuildInputs
  ++ pkgs.default.propagatedBuildInputs;
}
