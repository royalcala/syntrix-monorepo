{ config, pkgs, lib, ... }: {
  services.caddy = {
    enable = true;
    virtualHosts = {
      "api.syntrix.mx" = {
        extraConfig = ''
          reverse_proxy localhost:3001
          encode zstd gzip
        '';
      };
      "operator.syntrix.mx" = {
        extraConfig = ''
          root * /var/www/operator/dist
          file_server
          encode zstd gzip
        '';
      };
      "analytics.syntrix.mx" = {
        extraConfig = ''
          reverse_proxy localhost:8000
        '';
      };
    };
  };
}
