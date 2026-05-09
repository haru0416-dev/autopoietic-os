{ config, lib, ... }:

let
  cfg = config.autopoietic;
in
{
  options.autopoietic = {
    enable = lib.mkEnableOption "Autopoietic OS core";

    identity = {
      host = lib.mkOption {
        type = lib.types.str;
        default = "unknown";
        description = "Stable self-identity name for this host.";
      };

      roles = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = "Declared roles used by introspection and mutation planning.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    environment.etc."autopoietic/identity.json".text = builtins.toJSON {
      host = cfg.identity.host;
      roles = cfg.identity.roles;
    };
  };
}
