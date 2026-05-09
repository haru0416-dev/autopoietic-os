{ config, lib, ... }:

let
  cfg = config.autopoietic;
in
{
  options.autopoietic.observation = {
    projectRoots = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = "Project roots that os-introspect may summarize.";
    };

    includeShellHistoryByDefault = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether shell history should be included without an explicit CLI flag.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.etc."autopoietic/observation.json".text = builtins.toJSON {
      project_roots = cfg.observation.projectRoots;
      include_shell_history_by_default = cfg.observation.includeShellHistoryByDefault;
    };
  };
}
