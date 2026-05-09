# ISO boot verification — 2026-05-08, updated 2026-05-09

This note records executable Linux-level verification for the Autopoietic OS ISO configuration and production ISO artifact.

## Scope

The checks verify a test-instrumented ISO built from the same Autopoietic ISO host configuration and core module as `nixosConfigurations.iso`.

The test instrumentation is imported only by `tests/vm/iso-boot.nix` so the VM test driver can control the guest. It is not part of the production `packages.x86_64-linux.iso` output.

The test-instrumented ISO also includes `jq` so assertions can parse JSON structurally instead of matching text with `grep`. This is a test harness dependency, not a production ISO dependency.

The production ISO checks boot `nixosConfigurations.iso.config.system.build.isoImage` directly, without test instrumentation, and watch the serial console for `multi-user.target`. The ISO enables both `console=tty0` and `console=ttyS0,115200n8` so it remains usable on a normal display while also exposing a deterministic headless boot oracle.

ADR 0012 fixes this as the P0 verification boundary: detailed observe-only assertions are test-instrumented, while the production artifact is black-box boot-verified.

## External grounding

- External surface: locked nixpkgs NixOS VM test framework, QEMU boot command construction, installer ISO test instrumentation, and serial-console kernel parameters.
- Stale risk: Class A / B — NixOS test framework APIs, QEMU package attributes, OVMF firmware attributes, boot kernel parameters, and installer module behavior are version-sensitive and must match the pinned nixpkgs revision.
- Local anchor:
  - `flake.lock` pins nixpkgs revision `549bd84d6279f9852cae6225e372cc67fb91a4c1`.
  - Locked source path: `/nix/store/h9wn92hv33sizprch7fcp16lfs1k3w5j-source`.

## Claims

### EG-001

- Claim: A NixOS VM test can boot an installer ISO by creating a machine with QEMU `-cdrom`, then use `machine.wait_for_unit("multi-user.target")` and `machine.succeed(...)` assertions.
- Source: locked nixpkgs `/nix/store/h9wn92hv33sizprch7fcp16lfs1k3w5j-source/nixos/tests/boot.nix` lines 88-107 and 176-179.
- Source quality: local locked nixpkgs source.
- Version fit: matches `flake.lock` nixpkgs revision.
- Executable probe: `nix build path:/home/haru/OS#checks.x86_64-linux.iso-boot-basic --no-link --print-out-paths` produced `/nix/store/svyz3pvdn5cflh8bf3pa2r6c60h4ay83-vm-test-run-autopoietic-iso-boot-basic`.
- Lateral check: locked `nixos/lib/testing-python.nix` exposes `makeTest` and the VM test log shows `machine.wait_for_unit("multi-user.target")` completed.
- Decision: confirmed.

### EG-002

- Claim: The Autopoietic ISO host configuration boots to `multi-user.target` and provides detailed observe-only Autopoietic configuration, initialized memory directories, disabled live agent runtime, working introspection, and mutation journal behavior inside the guest.
- Source: local `tests/vm/iso-boot.nix`, `hosts/iso/configuration.nix`, and VM test execution log.
- Source quality: local project source plus executable VM probe.
- Version fit: matches the locked flake inputs and local Autopoietic module source.
- Executable probes:
  - `nix build path:/home/haru/OS#checks.x86_64-linux.iso-observe-only --no-link --print-out-paths` produced `/nix/store/f0i2k3qnmc6chpirygpl9wj1s6ayjzyl-vm-test-run-autopoietic-iso-observe-only`.
  - `nix build path:/home/haru/OS#checks.x86_64-linux.iso-tools --no-link --print-out-paths` produced `/nix/store/wzykfvhcalyjd6dya8ndvhszqvvycz4j-vm-test-run-autopoietic-iso-tools`.
- Lateral check: `nix flake check --no-write-lock-file path:/home/haru/OS` reran `iso-boot-basic`, `iso-observe-only`, `iso-tools`, `iso-uefi-boot`, and the `iso-boot` alias, then reported `all checks passed`; the focused checks record:
  - `Reached target Multi-User System.`
  - structural `jq` identity checks for `host = autopoietic-iso` and roles `installer`, `live`, and `observe-only`
  - structural `jq` observation policy checks for `/etc/nixos`, `/mnt/etc/nixos`, and disabled default shell history inclusion
  - structural `jq` mutation runner checks for `mode = observe-only` and `memory_dir = /var/lib/autopoietic`
  - successful `/var/lib/autopoietic` directory initialization checks for `mutations`, `effects`, `generations`, and `organs`, including `root:root:750` permissions on the memory root
  - successful absence check for `autopoietic-agent.service`
  - successful `command -v os-introspect`, `command -v mutation-journal`, and `command -v mutation-runner`
  - successful `os-introspect --root /etc --output /tmp/self-state.json`
  - structural `jq` self-state checks for `schema_version`, `identity`, `genome`, `body.systemd`, and `memory`
  - successful `mutation-journal append --path /tmp/mutations.jsonl ...`
  - structural `jq` JSONL checks for one line with `goal = iso boot smoke`, `status = accepted`, and `phase = P0-smoke`
  - successful negative check that malformed metadata is rejected with `metadata must be key=value`
- Decision: confirmed for the test-instrumented ISO variant.

### EG-003

- Claim: The locked nixpkgs test framework can boot the instrumented Autopoietic ISO as a UEFI CD-ROM by adding pflash drives for `pkgs.OVMF.firmware` and `pkgs.OVMF.variables` to the QEMU start command.
- Source: locked nixpkgs `/nix/store/h9wn92hv33sizprch7fcp16lfs1k3w5j-source/nixos/tests/boot.nix` lines 53-58 and 162-165.
- Source quality: local locked nixpkgs source.
- Version fit: matches `flake.lock` nixpkgs revision.
- Executable probe: `nix build path:/home/haru/OS#checks.x86_64-linux.iso-uefi-boot --no-link --print-out-paths` produced `/nix/store/yy37hzwjiqh0z70h3kkap008pw5air07-vm-test-run-autopoietic-iso-uefi-boot`.
- Lateral check: locked nixpkgs `/nix/store/h9wn92hv33sizprch7fcp16lfs1k3w5j-source/pkgs/applications/virtualization/OVMF/default.nix` lines 249-256 exposes the `firmware` and `variables` passthru paths used by the QEMU pflash drives.
- Decision: confirmed for the test-instrumented ISO variant.

### EG-004

- Claim: The production Autopoietic ISO artifact can boot under QEMU BIOS CD-ROM without NixOS test instrumentation and reach `multi-user.target`, with the test observing that state from the serial console.
- Source: local `tests/vm/iso-boot.nix` production console check, `hosts/iso/configuration.nix` serial kernel parameters, and VM test execution log.
- Source quality: local project source plus executable VM probe.
- Version fit: matches the locked flake inputs and local Autopoietic ISO configuration.
- Executable probe: `nix build path:/home/haru/OS#checks.x86_64-linux.iso-production-boot-console --no-link --print-out-paths` produced `/nix/store/h3p589sa8bs9cwkzlz5mwjsaffx4c32v-vm-test-run-autopoietic-iso-production-boot-console`.
- Lateral check: the first attempt without serial kernel parameters booted the ISO but did not expose post-kernel progress to the serial console; after adding `console=tty0` and `console=ttyS0,115200n8`, the execution log recorded `Reached target Multi-User System`, `Serial Getty on ttyS0`, and the `autopoietic-iso login` prompt without importing test instrumentation.
- Decision: confirmed for BIOS CD-ROM boot of the production ISO artifact.

### EG-005

- Claim: The production Autopoietic ISO artifact can boot under QEMU UEFI CD-ROM without NixOS test instrumentation and reach `multi-user.target`, with the test observing that state from the serial console.
- Source: local `tests/vm/iso-boot.nix` production UEFI console check, `hosts/iso/configuration.nix` serial kernel parameters, locked nixpkgs OVMF attributes, and VM test execution log.
- Source quality: local project source plus executable VM probe.
- Version fit: matches the locked flake inputs and local Autopoietic ISO configuration.
- Executable probe: `nix build path:/home/haru/OS#checks.x86_64-linux.iso-production-uefi-boot-console --no-link --print-out-paths` produced `/nix/store/lxsn8l3hs8w646n4n1vzsx2bdnblw1dg-vm-test-run-autopoietic-iso-production-uefi-boot-console`.
- Lateral check: `nix flake check --no-write-lock-file path:/home/haru/OS` includes the production UEFI console check, and EG-003 independently grounds the OVMF pflash paths used for UEFI boot.
- Decision: confirmed for UEFI CD-ROM boot of the production ISO artifact.

## Verification commands

From an uncommitted Git worktree, use an explicit path flake so Nix sees untracked files:

```bash
nix build path:/home/haru/OS#checks.x86_64-linux.iso-boot-basic --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-observe-only --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-tools --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-uefi-boot --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-production-boot-console --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-production-uefi-boot-console --no-link --print-out-paths
```

After files are staged or committed, this can be run as:

```bash
nix build .#checks.x86_64-linux.iso-boot-basic --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-observe-only --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-tools --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-uefi-boot --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-production-boot-console --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-production-uefi-boot-console --no-link --print-out-paths
```

For compatibility with earlier notes, `checks.x86_64-linux.iso-boot` remains an alias for the basic BIOS CD-ROM boot check.

## Result

The focused VM boot checks passed after the split:

- `iso-boot-basic` reached `multi-user.target` under BIOS CD-ROM boot and structurally verified `/etc/autopoietic/identity.json` with `jq`.
- `iso-observe-only` structurally verified observation policy and mutation runner mode, verified memory directories including `generations`, and confirmed the live agent runtime is not enabled by default.
- `iso-tools` ran `os-introspect`, confirmed `mutation-runner` is present, structurally checked the emitted self-state JSON, appended a mutation journal entry to `/tmp`, structurally checked the JSONL entry, and rejected malformed metadata.
- `iso-uefi-boot` reached the observe-only system under UEFI CD-ROM boot with OVMF pflash firmware.
- `iso-production-boot-console` booted the production ISO artifact without test instrumentation and observed `multi-user.target` on the serial console.
- `iso-production-uefi-boot-console` booted the production ISO artifact under UEFI without test instrumentation and observed `multi-user.target` on the serial console.

## Remaining caveat

The detailed observe-only assertions still run against the test-instrumented ISO because they require a guest command channel. The production ISO check is intentionally narrower: it is a black-box artifact boot check that proves the distributable ISO reaches `multi-user.target` by default. Future production-artifact checks can expand this by adding a read-only serial status oracle for Autopoietic configuration state, without importing the NixOS test backdoor.
