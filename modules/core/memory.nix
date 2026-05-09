{ config, lib, ... }:

let
  cfg = config.autopoietic;
in
{
  options.autopoietic.memory = {
    dir = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/autopoietic";
      description = "Persistent directory for mutation, generation, organ, and effect ledgers.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.tmpfiles.rules = [
      "d ${cfg.memory.dir} 0750 root root - -"
      "d ${cfg.memory.dir}/mutations 0750 root root - -"
      "d ${cfg.memory.dir}/effects 0750 root root - -"
      "d ${cfg.memory.dir}/generations 0750 root root - -"
      "d ${cfg.memory.dir}/organs 0750 root root - -"
    ];
  };
}
