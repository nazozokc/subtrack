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
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      perSystem =
        { system, ... }:
        let
          pkgs = import inputs.nixpkgs { inherit system; };
        in
        {
          formatter = pkgs.nixfmt-rfc-style;

          devShells = {
            default = pkgs.mkShell {
              name = "subtrack-dev";
              packages = with pkgs; [
                nodejs
                pnpm
                typos
                typescript
                nixfmt-rfc-style
              ];
              shellHook = ''
                echo "[subtrack devShell]"
                echo "  node $(node --version)  pnpm $(pnpm --version)"
                echo ""
                echo "  Tasks:"
                echo "    pnpm install    install dependencies"
                echo "    pnpm build      build packages"
                echo "    pnpm test       run tests"
                echo "    pnpm start      run CLI (dev mode)"
                echo "    typos           check typos"
                echo "    nix fmt         format nix files"
              '';
            };

            ci = pkgs.mkShell {
              name = "subtrack-ci";
              packages = with pkgs; [
                nodejs
                pnpm
              ];
            };
          };
        };
    };
}
