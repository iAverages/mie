{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-linux"] (system: let
      version = "0.1.0";
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {
        inherit system overlays;
      };

      buildDeps = with pkgs; [
        pkg-config
        perl
      ];

      devDeps = with pkgs; [
        gcc
        just
        (rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
        })
      ];
      runtimeDeps = with pkgs; [
        openssl
        ffmpeg
        cacert
      ];
    in {
      packages = {
        default = pkgs.rustPlatform.buildRustPackage {
          inherit version;
          pname = "mie";
          src = ./.;
          # cargoLock = {
          #   lockFile = ./Cargo.lock;
          # };
          cargoDeps = pkgs. rustPlatform.importCargoLock {
            lockFile = ./Cargo.lock;
          };
          nativeBuildInputs = buildDeps;
          buildInputs = runtimeDeps;
        };
        dockerImage = pkgs.dockerTools.buildLayeredImage {
          name = "mie";
          tag = version;
          config.Cmd = ["${self.packages.${system}.default}/bin/mie"];
          contents =
            [
              self.packages.${system}.default
            ]
            ++ runtimeDeps;
        };
      };

      devShells = {
        default = pkgs.mkShell {
          shellHook = ''
            export  PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig";
          '';
          packages = devDeps ++ runtimeDeps ++ buildDeps;
        };
      };
    });
}
