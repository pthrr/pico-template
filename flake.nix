{
  description = "cpp-template";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, }: utils.lib.eachDefaultSystem (system: let
    pkgs = import nixpkgs { inherit system; };
  in {
    devShells.default = pkgs.mkShell.override { stdenv = pkgs.gcc13Stdenv; } {
      packages = with pkgs; [
        cmake
        conan
        ninja
        mold
        sccache
        python3
        gdb
      ];

      shellHook = ''
        export IN_NIX_SHELL=1
      '';
    };
  });
}
