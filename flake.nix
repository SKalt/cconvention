{
  description = "A language server to help write conventional commits.";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils"; # TODO: pin
    rust-overlay = {
      url = "github:oxalica/rust-overlay"; # TODO: pin
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = { flake-utils, nixpkgs, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [(import rust-overlay)];
        pkgs = (import nixpkgs) {
          inherit system overlays;
        };
        rust_toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        cargoDeps = pkgs.rustPlatform.importCargoLock {
          lockFile = ./Cargo.lock;
        };
        base_info = (builtins.fromTOML (builtins.readFile ./pkg/base/Cargo.toml));
        # pro_info = (builtins.fromTOML (builtins.readFile ./pkg/pro/Cargo.toml));
      in {
        # TODO: figure out how to build or cross-compile
        packages = {
          # For `nix build` & `nix run`:
          # TODO: base, pro, default
          base = pkgs.rustPlatform.buildRustPackage
            {
              # https://nixos.org/manual/nixpkgs/stable/#compiling-rust-applications-with-cargo
              inherit cargoDeps;
              cargoLock = { # TODO: see if this is needed
                lockFile = ./Cargo.lock;
              };
              pname = "cconvention"; # overrides actual info in pkg/base/Cargo.toml
              version = base_info.package.version;
              src = ./.;
              # > One caveat is that Cargo.lock cannot be patched in the patchPhase because it runs after the dependencies have already been fetched.
              # > Note that setting cargoLock.lockFile or cargoLock.lockFileContents doesn’t add a Cargo.lock to your src, and a Cargo.lock is still required to build a rust package.
              # > -- https://nixos.org/manual/nixpkgs/stable/#importing-a-cargo.lock-file
              # postPatch = ''
              #   ln -s ${./Cargo.lock} Cargo.lock
              # '';

              nativeBuildInputs = [rust_toolchain];
            };

        };
        # For `nix develop`:
        devShell = pkgs.mkShell {
          # see https://github.com/NixOS/nixpkgs/issues/52447
          # see https://hoverbear.org/blog/rust-bindgen-in-nix/
          # see https://slightknack.dev/blog/nix-os-bindgen/
          # https://nixos.wiki/wiki/Rust#Installation_via_rustup
          nativeBuildInputs = [rust_toolchain];
          buildInputs = with pkgs;
            [
              # rust tools
              cargo-bloat
              rust-analyzer-unwrapped
              cargo-cross
              zig # temp
              cargo-zigbuild
              llvmPackages_16.bintools

              # nix support
              nixpkgs-fmt
              nil

              # for recording demos
              vhs
              ttyd
              ffmpeg
              libfaketime
              git
              bashInteractive

              # JS/TS development
              nodejs_18
              nodejs_18.pkgs.pnpm

              # demo editors
              helix
              vim
              neovim

              # other
              lychee
              shellcheck
            ];
        };

        # Environment variables
        RUST_SRC_PATH = "${rust_toolchain}/lib/rustlib/src/rust/library";
      }
    );
}
