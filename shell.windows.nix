{ pkgs ? import (builtins.fetchTarball "https://github.com/NixOS/nixpkgs/archive/nixos-24.11.tar.gz") {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/470ee44393bb19887056b557ea2c03fc5230bd5a.tar.gz"))
    ];
  }
}:
(import ./nix/dev-shells.nix { inherit pkgs; }).windows
