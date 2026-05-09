{ pkgs, ... }:

{
  imports = [ ./hardware-configuration.nix ];

  networking.hostName = "aion";

  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  autopoietic = {
    enable = true;
    identity = {
      host = "aion";
      roles = [ "development" "research" "writing" ];
    };
    memory.dir = "/var/lib/autopoietic";
    observation.projectRoots = [ "/home/user/projects" ];
  };

  users.users.user = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
    packages = [ pkgs.git ];
  };

  home-manager.useGlobalPkgs = true;
  home-manager.useUserPackages = true;
  home-manager.users.user = import ../../home/user.nix;

  system.stateVersion = "25.05";
}
