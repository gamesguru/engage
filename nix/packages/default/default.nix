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
      $out/bin/${cmd} completions bash > bash_comp
      sed -i '/-p\|--process)/,/;;/ s|COMPREPLY=($(compgen -f "''${cur}"))|COMPREPLY=($(compgen -W "$(${cmd} list --relaxed 2>/dev/null)" -- "''${cur}"))|' bash_comp

      $out/bin/${cmd} completions zsh > zsh_comp
      sed -i 's|:PROCESS:_default|:PROCESS:($(${cmd} list --relaxed 2>/dev/null))|' zsh_comp

      $out/bin/${cmd} completions fish > fish_comp
      sed -i 's|-s p -l process -d '\''Select a process'\'' -r|-s p -l process -d '\''Select a process'\'' -f -a "(${cmd} list --relaxed 2>/dev/null)"|' fish_comp

      installShellCompletion --cmd ${cmd} \
        --bash bash_comp \
        --zsh zsh_comp \
        --fish fish_comp \
        --elvish <($out/bin/${cmd} completions elvish) \
        --powershell <($out/bin/${cmd} completions powershell)

      ${lib.getExe mdbook} build
      mkdir -p "$doc/share/doc"
      mv public "$doc/share/doc/${crateName.pname}"
    '';

  meta.mainProgram = crateName.pname;
}
