{ config, lib, ... }:

let
  cfg = config.autopoietic;
in
{
  options.autopoietic.mutationRunner = {
    mode = lib.mkOption {
      type = lib.types.enum [ "observe-only" "mutate-draft" "mutate-vm" "mutate-live" "autopoiesis" ];
      default = "observe-only";
      description = "Maximum mutation autonomy permitted for this host.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.etc."autopoietic/mutation-runner.json".text = builtins.toJSON {
      mode = cfg.mutationRunner.mode;
      memory_dir = cfg.memory.dir;
    };
  };
}
