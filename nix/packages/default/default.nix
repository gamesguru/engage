# Keep sorted.
{
  craneLib,
  installShellFiles,
  lib,
  mdbook,
}:

let
  crateName = craneLib.crateNameFromCargoToml {
    cargoToml = ../../../crates/engage/Cargo.toml;
  };
in

craneLib.buildPackage {
  inherit (crateName) pname version;

  outputs = [ "out" "doc" ];

  env = {
    ENGAGE_DOCS_LINK = "file://"
      + (builtins.placeholder "doc")
      + "/share/doc/"
      + crateName.pname
      + "/index.html";
  };

  src =
    let
      inherit (lib.fileset) unions toSource;
    in
    toSource {
      root = ../../..;

      # Keep sorted.
      fileset = unions [
        ../../../Cargo.lock
        ../../../Cargo.toml
        ../../../README.md
        ../../../book
        ../../../book.toml
        ../../../crates
      ];
    };

  nativeBuildInputs = [
    installShellFiles
  ];

  # This is more or less redundant with and less extensive than CI.
  doCheck = false;

  postInstall =
    let
      cmd = crateName.pname;
    in
    ''
      installShellCompletion --cmd ${cmd} ${
        builtins.concatStringsSep
          " "
          (builtins.map
            (shell: "--${shell} <($out/bin/${cmd} completions ${shell})")
            [ "bash" "zsh" "fish" ]
          )
      }

      ${lib.getExe mdbook} build
      mkdir -p "$doc/share/doc"
      mv public "$doc/share/doc/${crateName.pname}"
    '';

  meta.mainProgram = crateName.pname;
}
