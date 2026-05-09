{ ... }:

{
  # Evaluation-only placeholder for the seed host.
  # Replace this file with nixos-generate-config output before installing on real hardware.
  boot.loader.grub.enable = false;
  fileSystems."/" = {
    device = "nodev";
    fsType = "none";
  };
}
