{ pkgs, ... }:

{
  home.username = "user";
  home.homeDirectory = "/home/user";

  home.packages = [
    pkgs.ripgrep
    pkgs.fd
  ];

  programs.git.enable = true;

  home.stateVersion = "25.05";
}
