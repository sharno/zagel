{
  description = "Zagel: Rust GUI REST workbench with Nix flake checks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        nativeBuildDeps = with pkgs; [ pkg-config ];
        runtimeDeps = with pkgs; [
          udev
          wayland
          libxkbcommon
          libx11
          libxcursor
          libxi
          libxinerama
          libxrandr
          libxcb
          libxcb-util
        ];
        zagel = pkgs.rustPlatform.buildRustPackage {
          pname = "zagel";
          version = "0.1.0";
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = nativeBuildDeps;
          buildInputs = runtimeDeps;
          doCheck = true;
        };
      in
      {
        packages.default = zagel;
        packages.zagel = zagel;

        checks.default = zagel;

        devShells.default = pkgs.mkShell {
          packages =
            with pkgs;
            [
              cargo
              clippy
              rustc
              rustfmt
            ]
            ++ nativeBuildDeps
            ++ runtimeDeps;
          env.RUST_BACKTRACE = "1";
        };

        formatter = pkgs.nixfmt;
      }
    );
}
