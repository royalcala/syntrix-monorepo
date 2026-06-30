{ config, pkgs, lib, ... }: {
  services.plausible = {
    enable = true;
    server = {
      port = 8000;
      baseUrl = "https://analytics.syntrix.mx";
      secretKeyBase = "CHANGE_ME_PLAUSIBLE_SECRET";
    };
    adminUser = {
      name = "Admin";
      email = "admin@syntrix.mx";
      password = "CHANGE_ME_ADMIN_PASSWORD";
    };
    database = {
      url = "postgres://syntrix:password@localhost:5432/syntrix";
    };
  };
}
