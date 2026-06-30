{
  description = "Syntrix infrastructure — NixOS declarative configs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    colmena.url = "github:zhaofengli/colmena";
    colmena.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, colmena, ... } @ inputs: {
    colmena = {
      meta = {
        nixpkgs = import nixpkgs { system = "x86_64-linux"; };
        specialArgs = { inherit inputs; };
      };

      hetzner = { name, nodes, ... }: {
        imports = [
          ./hosts/hetzner/configuration.nix
          ./modules/syntrix-backend.nix
          ./modules/caddy.nix
          ./modules/postgresql.nix
          ./modules/plausible.nix
        ];

        networking.hostName = "hetzner";
        networking.domain = "syntrix.mx";

        system.stateVersion = "24.11";
      };
    };
  };
}
