{
  description = "A CLI tool to manage your subscription services from the terminal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    { flake-parts, ... }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      perSystem =
        { system, ... }:
        let
          pkgs = import inputs.nixpkgs { inherit system; };
        in
        {
          devShells.default = pkgs.mkShell {
            name = "subtrack-dev";
            packages = with pkgs; [
              typos
              nodejs
              pnpm
            ];
            shellHook = ''
              echo "[subtrack devShell] typos available"
            '';
          };
        };
    };
}
