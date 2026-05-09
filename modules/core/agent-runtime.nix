{ config, lib, ... }:

let
  cfg = config.autopoietic;
in
{
  options.autopoietic.agentRuntime = {
    enable = lib.mkEnableOption "the autopoietic agent runtime service";
    command = lib.mkOption {
      type = lib.types.str;
      default = "sleep infinity";
      description = "Command used by the placeholder agent runtime.";
    };
  };

  config = lib.mkIf (cfg.enable && cfg.agentRuntime.enable) {
    systemd.services.autopoietic-agent = {
      description = "Autopoietic NixOS agent runtime";
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = cfg.agentRuntime.command;
        Restart = "on-failure";
      };
    };
  };
}
