{
  pkgs,
  lib,
  version,
  buildDeps,
  runtimeDeps,
  ...
}:
pkgs.rustPlatform.buildRustPackage {
  inherit version;
  pname = "mie-api";
  src = lib.cleanSourceWith {
    src = ../../.;
    filter = path: type: let
      relPath = lib.removePrefix (toString ../../. + "/") (toString path);
    in
      lib.any (p: lib.hasPrefix p relPath) [
        "bot"
        "api"
        "shared"
        ".cargo"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };
  cargoLock = {
    lockFile = ../../Cargo.lock;
  };

  # only build the api
  cargoBuildFlags = [
    "--bin"
    "api"
  ];

  nativeBuildInputs = buildDeps;
  buildInputs = runtimeDeps;
}
