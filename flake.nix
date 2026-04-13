# flake.nix
{
  description = "Development environment for ghostlayer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" ];
          targets = [
            "aarch64-apple-ios"       # iPhone + iPad (physisch, Apple Silicon)
            "aarch64-apple-ios-sim"   # Simulator (Apple Silicon Mac)
            "x86_64-apple-ios"        # Simulator (Intel Mac)
            "aarch64-apple-ios-macabi" # Mac Catalyst (Apple Silicon)
            "x86_64-apple-ios-macabi"  # Mac Catalyst (Intel)
            "aarch64-apple-darwin"    # macOS (Apple Silicon)
            "x86_64-apple-darwin"     # macOS (Intel)
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-udeps
            pkg-config
            jujutsu

            # Needed for image processing / FFI if OS libraries are linked
            libiconv
          ];

          shellHook = ''
            echo "GhostLayer Development Shell"
            echo "Rust: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
            echo "Jujutsu: $(jj --version)"
          '';
        };
      }
    );
}
