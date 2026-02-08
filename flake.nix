{
  description = "bakopa-vr dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
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
          devShellDefs = import ./nix/dev-shells.nix { inherit pkgs; };
        in
        {
          default = devShellDefs.default;
        } // nixpkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          windows = devShellDefs.windows;
        });
    };
}
