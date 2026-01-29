{
  description = "Semantic CLI tool for process management. Target by port, PID, name or path.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "proc";
          version = "1.3.3";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          
          meta = with pkgs.lib; {
            description = "Semantic CLI tool for process management. Target by port, PID, name or path.";
            homepage = "https://github.com/yazeed/proc";
            license = licenses.mit;
            maintainers = [];
            mainProgram = "proc";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustc
            cargo
            rust-analyzer
          ];
        };
      }
    );
}
