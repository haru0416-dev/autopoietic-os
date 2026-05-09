{
  autopoieticModule,
  autopoieticTools,
  nixpkgs,
  productionIso,
  system,
}:

let
  pkgs = import nixpkgs { inherit system; };
  lib = pkgs.lib;
  testing = import "${nixpkgs}/nixos/lib/testing-python.nix" { inherit system pkgs; };
  qemu-common = import "${nixpkgs}/nixos/lib/qemu-common.nix" {
    inherit (pkgs) lib stdenv;
  };
  testIso =
    (nixpkgs.lib.nixosSystem {
      inherit system;
      specialArgs = {
        inherit autopoieticTools;
      };
      modules = [
        "${nixpkgs}/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix"
        "${nixpkgs}/nixos/modules/testing/test-instrumentation.nix"
        ../../hosts/iso/configuration.nix
        autopoieticModule
        {
          environment.systemPackages = [ pkgs.jq ];
          nixpkgs.pkgs = pkgs;
        }
      ];
    }).config.system.build.isoImage;
  qemu = qemu-common.qemuBinary pkgs.qemu_test;
  testIsoPath = "${testIso}/iso/${testIso.isoName}";
  productionIsoPath = "${productionIso}/iso/${productionIso.isoName}";
  mkStartCommand =
    {
      isoPath ? testIsoPath,
      uefi ? false,
    }:
    let
      flags = [
        qemu
        "-m"
        "2048"
        "-netdev"
        "user,id=net0"
        "-device"
        "virtio-net-pci,netdev=net0"
        "-cdrom"
        isoPath
      ]
      ++ lib.optionals uefi [
        "-drive"
        "if=pflash,format=raw,unit=0,readonly=on,file=${pkgs.OVMF.firmware}"
        "-drive"
        "if=pflash,format=raw,unit=1,readonly=on,file=${pkgs.OVMF.variables}"
      ];
    in
    lib.concatStringsSep " " flags;
  mkIsoBootTest =
    {
      name,
      uefi ? false,
      assertions,
    }:
    testing.makeTest {
      name = "autopoietic-iso-${name}";
      nodes = { };

      testScript = ''
        machine = create_machine("${mkStartCommand { inherit uefi; }}")
        machine.start()
        machine.wait_for_unit("multi-user.target")

        ${assertions}

        machine.shutdown()
      '';
    };
in
{
  boot-basic = mkIsoBootTest {
    name = "boot-basic";
    assertions = ''
      with subtest("Autopoietic identity is present"):
          machine.succeed("test -f /etc/autopoietic/identity.json")
          machine.succeed("jq -e '.host == \"autopoietic-iso\"' /etc/autopoietic/identity.json")
          machine.succeed("jq -e '.roles == [\"installer\", \"live\", \"observe-only\"]' /etc/autopoietic/identity.json")
    '';
  };

  observe-only = mkIsoBootTest {
    name = "observe-only";
    assertions = ''
      with subtest("Autopoietic observation policy is present"):
          machine.succeed("test -f /etc/autopoietic/observation.json")
          machine.succeed("jq -e '.project_roots == [\"/etc/nixos\", \"/mnt/etc/nixos\"]' /etc/autopoietic/observation.json")
          machine.succeed("jq -e '.include_shell_history_by_default == false' /etc/autopoietic/observation.json")

      with subtest("Mutation runner is observe-only"):
          machine.succeed("test -f /etc/autopoietic/mutation-runner.json")
          machine.succeed("jq -e '.mode == \"observe-only\"' /etc/autopoietic/mutation-runner.json")
          machine.succeed("jq -e '.memory_dir == \"/var/lib/autopoietic\"' /etc/autopoietic/mutation-runner.json")

      with subtest("Memory directories are initialized by tmpfiles"):
          machine.succeed("test -d /var/lib/autopoietic")
          machine.succeed("test -d /var/lib/autopoietic/mutations")
          machine.succeed("test -d /var/lib/autopoietic/effects")
          machine.succeed("test -d /var/lib/autopoietic/generations")
          machine.succeed("test -d /var/lib/autopoietic/organs")
          machine.succeed("test \"$(stat -c '%U:%G:%a' /var/lib/autopoietic)\" = root:root:750")

      with subtest("No live agent runtime is enabled by default"):
          machine.succeed("if systemctl cat autopoietic-agent.service >/tmp/autopoietic-agent.unit 2>/dev/null; then cat /tmp/autopoietic-agent.unit; exit 1; fi")
    '';
  };

  tools = mkIsoBootTest {
    name = "tools";
    assertions = ''
      with subtest("Autopoietic tools run inside the booted ISO"):
          machine.succeed("command -v os-introspect")
          machine.succeed("command -v mutation-journal")
          machine.succeed("command -v mutation-runner")
          machine.succeed("os-introspect --root /etc --output /tmp/self-state.json")
          machine.succeed("test -s /tmp/self-state.json")
          machine.succeed("jq -e '.schema_version == \"0.1.0\"' /tmp/self-state.json")
          machine.succeed("jq -e '.identity.host == \"autopoietic-iso\"' /tmp/self-state.json")
          machine.succeed("jq -e '.genome.has_flake == false' /tmp/self-state.json")
          machine.succeed("jq -e '.body.systemd.available == true' /tmp/self-state.json")
          machine.succeed("jq -e '.memory.root == \"/etc/memory\"' /tmp/self-state.json")
          machine.succeed("mutation-journal append --path /tmp/mutations.jsonl --goal 'iso boot smoke' --status accepted --phase P0-smoke --reason 'booted instrumented ISO' --changed-path /etc/autopoietic/mutation-runner.json")
          machine.succeed("test -s /tmp/mutations.jsonl")
          machine.succeed("test \"$(wc -l < /tmp/mutations.jsonl)\" = 1")
          machine.succeed("jq -e '.goal == \"iso boot smoke\" and .status == \"accepted\" and .phase == \"P0-smoke\"' /tmp/mutations.jsonl")

      with subtest("Mutation journal rejects malformed metadata"):
          machine.fail("mutation-journal append --path /tmp/bad-mutations.jsonl --goal bad-metadata --phase P0-smoke --metadata malformed")
    '';
  };

  uefi-boot = mkIsoBootTest {
    name = "uefi-boot";
    uefi = true;
    assertions = ''
      with subtest("UEFI boot reaches the observe-only system"):
          machine.succeed("test -f /etc/autopoietic/mutation-runner.json")
          machine.succeed("jq -e '.mode == \"observe-only\"' /etc/autopoietic/mutation-runner.json")
    '';
  };

  production-boot-console = testing.makeTest {
    name = "autopoietic-iso-production-boot-console";
    nodes = { };

    testScript = ''
      import re

      machine = create_machine("${mkStartCommand { isoPath = productionIsoPath; }}")
      machine.start()

      def reached_multi_user(last_try):
          console = machine.get_console_log()
          if last_try:
              log.info(console)
          return re.search("Reached target.*Multi-User System", console) is not None

      retry(reached_multi_user, timeout_seconds=180)
      machine.crash()
    '';
  };

  production-uefi-boot-console = testing.makeTest {
    name = "autopoietic-iso-production-uefi-boot-console";
    nodes = { };

    testScript = ''
      import re

      machine = create_machine("${mkStartCommand { isoPath = productionIsoPath; uefi = true; }}")
      machine.start()

      def reached_multi_user(last_try):
          console = machine.get_console_log()
          if last_try:
              log.info(console)
          return re.search("Reached target.*Multi-User System", console) is not None

      retry(reached_multi_user, timeout_seconds=180)
      machine.crash()
    '';
  };
}
