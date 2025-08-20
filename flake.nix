{
  description = "Sftool is a download tool for the SiFli family of chips";

  outputs =
    {
      nixpkgs,
      ...
    }:
    let
      systems = [
        "aarch64-linux"
        "i686-linux"
        "x86_64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system: {
        sftool = nixpkgs.legacyPackages.${system}.callPackage ./nix/sftool.nix { };
      });
    };
}
