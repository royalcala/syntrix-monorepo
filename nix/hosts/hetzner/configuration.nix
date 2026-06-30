{ lib, pkgs, ... }: {
  boot.loader.grub.enable = true;
  boot.loader.grub.device = "/dev/sda";

  networking.firewall.allowedTCPPorts = [ 80 443 3001 ];

  services.openssh.enable = true;
  users.users.root.openssh.authorizedKeys.keys = [
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIL0xFIXME" # placeholder — replace with real key
  ];

  environment.systemPackages = with pkgs; [
    curl
    htop
    vim
    git
    colmena
  ];

  systemd.tmpfiles.rules = [
    "d /mnt/storage-box 0755 root root -"
  ];

  fileSystems."/mnt/storage-box" = {
    device = "//hetzner-storage-box.your-server.de/backup";
    fsType = "cifs";
    options = [
      "username=placeholder-user"
      "password=placeholder-pass"
      "uid=root"
      "gid=root"
      "iocharset=utf8"
      "noexec"
      "nofail"
      "x-systemd.automount"
      "_netdev"
    ];
  };
}
