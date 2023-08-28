{
  description = "A language server to help write conventional commits.";
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.05";
  };

  outputs = { self, flake-utils, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };
        cargoDeps = pkgs.rustPlatform.importCargoLock {
          lockFile = ./Cargo.lock;
        };

      in
      rec {
        # TODO: figure out how to build or cross-compile
        # packages = {
        #   # For `nix build` & `nix run`:
        #   # TODO: base, pro
        #   base = pkgs.rustPlatform.buildRustPackage
        #     {
        #       # https://nixos.org/manual/nixpkgs/stable/#compiling-rust-applications-with-cargo
        #       inherit cargoDeps;
        #       cargoLock = {
        #         lockFile = ./Cargo.lock;
        #       };
        #       pname = "cconvention";
        #       version = (builtins.fromTOML (builtins.readFile ./pkg/base/Cargo.toml))."package".version;
        #       src = ./.;
        #       # > One caveat is that Cargo.lock cannot be patched in the patchPhase because it runs after the dependencies have already been fetched.
        #       # > Note that setting cargoLock.lockFile or cargoLock.lockFileContents doesn’t add a Cargo.lock to your src, and a Cargo.lock is still required to build a rust package.
        #       # > -- https://nixos.org/manual/nixpkgs/stable/#importing-a-cargo.lock-file
        #       # postPatch = ''
        #       #   ln -s ${./Cargo.lock} Cargo.lock
        #       # '';

        #       nativeBuildInputs = with pkgs;
        #         [
        #           cargo
        #           clippy
        #           rustc
        #         ];
        #     };
        # };
        # For `nix develop`:
        devShell = pkgs.mkShell {
          # see https://github.com/NixOS/nixpkgs/issues/52447
          # see https://hoverbear.org/blog/rust-bindgen-in-nix/
          # see https://slightknack.dev/blog/nix-os-bindgen/
          # https://nixos.wiki/wiki/Rust#Installation_via_rustup
          nativeBuildInputs = with pkgs; [
            cargo
            clippy
            rustup
            # rustc: omitted
          ];
          buildInputs = with pkgs;
            [
              # rust tools
              cargo-bloat
              rust-analyzer
              rustfmt
              cargo-cross

              # nix support
              nixpkgs-fmt
              rnix-lsp

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
        # From https://github.com/srid/rust-nix-template/blob/50741677232653ec0fb465471ce1ab83e37efb3a/flake.nix#L37

        shellHook = ''
          # For rust-analyzer 'hover' tooltips to work.
          export RUST_SRC_PATH=${pkgs.rustPlatform.rustLibSrc}
        '';

      }
    );
}
