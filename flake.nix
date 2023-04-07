{
  description = "TODO: add a description";
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, flake-utils, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };
        # Generate a user-friendly version number.
        version = builtins.substring 0 8 self.lastModifiedDate;
        cargoDeps = pkgs.rustPlatform.importCargoLock {
          lockFile = ./Cargo.lock;
        };
      in
      rec {
        packages = {
          # For `nix build` & `nix run`:
          default = pkgs.rustPlatform.buildRustPackage {
            # https://nixos.org/manual/nixpkgs/stable/#compiling-rust-applications-with-cargo
            inherit version;
            pname = "pg_walk";
            src = ./.;
            nativeBuildInputs = with pkgs; [
              cargo
              clippy
              rustc
            ];
          };
        };
        # For `nix develop`:
        devShell = pkgs.mkShell {
          # see https://github.com/NixOS/nixpkgs/issues/52447
          # see https://hoverbear.org/blog/rust-bindgen-in-nix/
          # see https://slightknack.dev/blog/nix-os-bindgen/
          # https://nixos.wiki/wiki/Rust#Installation_via_rustup
          nativeBuildInputs = with pkgs; [
            cargo
            clippy
            rustc
          ];
          buildInputs = with pkgs;
            [
              cargo-bloat
              nixpkgs-fmt
              pkgconfig
              rnix-lsp
              rust-analyzer
              rustfmt
              helix
            ];
        };
      }
    );
}
