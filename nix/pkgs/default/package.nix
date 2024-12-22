# Keep sorted.
{
  craneLib,
  installShellFiles,
  nix-filter,
  stdenv,
}:

let
  crateName = craneLib.crateNameFromCargoToml {
    cargoToml = ../../../Cargo.toml;
  };
in

craneLib.buildPackage {
  inherit stdenv;

  src = nix-filter {
    root = ../../..;

    # Keep sorted.
    include = [
      "Cargo.lock"
      "Cargo.toml"
      "assets"
      "src"
      "tests"
      "xtask"
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
