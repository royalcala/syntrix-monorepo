{ config, pkgs, lib, ... }: let
  backend = pkgs.rustPlatform.buildRustPackage {
    pname = "syntrix-backend";
    version = "0.1.0";
    src = ./../../apps/backend;
    cargoLock.lockFile = ./../../Cargo.lock;
    nativeBuildInputs = with pkgs; [ pkg-config openssl ];
    buildInputs = with pkgs; [ openssl postgresql ];
  };
in {
  systemd.services.syntrix-backend = {
    description = "Syntrix Managed Services Backend";
    wantedBy = [ "multi-user.target" ];
    after = [ "postgresql.service" "network.target" ];

    serviceConfig = {
      ExecStart = "${backend}/bin/syntrix-backend";
      Restart = "always";
      RestartSec = "5";
      User = "syntrix";
      Group = "syntrix";
      EnvironmentFile = "/etc/syntrix-backend.env";
      Environment = [
        "DATABASE_URL=postgres://syntrix:password@localhost:5432/syntrix"
        "RUST_LOG=syntrix_backend=info"
      ];
      StateDirectory = "syntrix-backend";
      StateDirectoryMode = "0750";
    };
  };

  users.users.syntrix = {
    isSystemUser = true;
    group = "syntrix";
    home = "/var/lib/syntrix-backend";
    createHome = true;
  };

  users.groups.syntrix = {};
}
