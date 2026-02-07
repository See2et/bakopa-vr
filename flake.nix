{
  description = "bakopa-vr dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
      devShellDefs = import ./nix/dev-shells.nix { inherit pkgs; };
    in
    {
      devShells.${system} = {
        default = devShellDefs.default;
        windows = devShellDefs.windows;
      };
    };
}
