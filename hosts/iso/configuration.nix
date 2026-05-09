{ autopoieticTools, pkgs, ... }:

{
  networking.hostName = "autopoietic-iso";

  boot.kernelParams = [
    "console=tty0"
    "console=ttyS0,115200n8"
  ];

  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  environment.systemPackages = [
    autopoieticTools
    pkgs.nano
  ];

  isoImage.edition = "autopoietic";

  autopoietic = {
    enable = true;
    identity = {
      host = "autopoietic-iso";
      roles = [ "installer" "live" "observe-only" ];
    };
    memory.dir = "/var/lib/autopoietic";
    observation.projectRoots = [ "/etc/nixos" "/mnt/etc/nixos" ];
    mutationRunner.mode = "observe-only";
  };
}
