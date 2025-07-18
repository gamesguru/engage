# Keep sorted.
{
  craneLib,
  installShellFiles,
  lib,
}:

let
  crateName = craneLib.crateNameFromCargoToml {
    cargoToml = ../../../Cargo.toml;
  };
in

craneLib.buildPackage {
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
        ../../../assets
        ../../../src
        ../../../xtask/Cargo.toml
        ../../../xtask/src
      ];
    };

  nativeBuildInputs = [
    installShellFiles
  ];

  postInstall =
    let
      cmd = crateName.pname;
    in
    "installShellCompletion --cmd ${cmd} " + builtins.concatStringsSep
      " "
      (builtins.map
        (shell: "--${shell} <($out/bin/${cmd} completions ${shell})")
        [ "bash" "zsh" "fish" ]
      );

  meta.mainProgram = crateName.pname;
}
