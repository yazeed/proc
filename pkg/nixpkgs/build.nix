# Builds package.nix (the file nixpkgs carries as pkgs/by-name/pr/proc-cli)
# against the nixpkgs in this repo's flake.lock, with the maintainer entry
# added in case that nixpkgs doesn't have it yet. Run from the repo root,
# mounted at /work in the nixos/nix image (RELEASE.md, "After the tag: nixpkgs").
let
  flake = builtins.getFlake "/work";
  pkgs = import flake.inputs.nixpkgs { };
  lib = pkgs.lib // {
    maintainers = {
      yazeed = {
        github = "yazeed";
        githubId = 191483;
        name = "Yazeed Al Oyoun";
      };
    }
    // pkgs.lib.maintainers;
  };
in
pkgs.callPackage ./package.nix { inherit lib; }
