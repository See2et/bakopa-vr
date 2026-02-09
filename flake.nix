{
  description = "bakopa-vr dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, nixpkgs-unstable, rust-overlay }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
    in
    {
      devShells = nixpkgs.lib.genAttrs supportedSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          pkgsUnstable = import nixpkgs-unstable {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          devShellDefs = import ./nix/dev-shells.nix {
            inherit pkgs pkgsUnstable;
          };
        in
        {
          default = devShellDefs.default;
        } // nixpkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          windows = devShellDefs.windows;
        });
    };
}
