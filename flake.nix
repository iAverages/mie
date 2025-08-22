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
        ffmpeg-headless
        cacert
        yt-dlp
      ];
    in {
      packages = {
        mie = pkgs.callPackage ./bot/nix/default.nix {inherit version buildDeps runtimeDeps;};
        mieDockerImage = pkgs.callPackage ./bot/nix/docker.nix {
          inherit version runtimeDeps;
          app = self.packages.${system}.mie;
        };

        api = pkgs.callPackage ./api/nix/default.nix {
          inherit version buildDeps;
          runtimeDeps = [];
        };
        apiDockerImage = pkgs.callPackage ./api/nix/docker.nix {
          inherit version;
          runtimeDeps = [];
          app = self.packages.${system}.api;
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
