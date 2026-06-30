{ config, pkgs, lib, ... }: {
  services.postgresql = {
    enable = true;
    ensureDatabases = [ "syntrix" ];
    ensureUsers = [{
      name = "syntrix";
      ensurePermissions = { "DATABASE syntrix" = "ALL PRIVILEGES"; };
    }];
    authentication = ''
      local syntrix syntrix trust
      host syntrix syntrix 127.0.0.1/32 trust
    '';
    settings = {
      max_connections = 50;
      shared_buffers = "256MB";
    };
  };

  services.postgresqlBackup = {
    enable = true;
    databases = [ "syntrix" ];
    location = "/mnt/storage-box/postgresql-backups";
    compression = "gz";
  };
}
