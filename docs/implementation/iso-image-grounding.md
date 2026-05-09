# ISO image grounding — 2026-05-08

This note records the external-grounding and semantic constraints used for the first Autopoietic NixOS ISO output.

## Lightweight semantic review

Module hypothesis: `flake.nix` is the genome entrypoint that exposes buildable organs and host configurations. `hosts/iso/configuration.nix` should be a minimal live/install media host layered on the locked NixOS minimal installer module, not a copy of the development host.

### Units

- `packages` / `apps` in `flake.nix`
  - What mechanical: builds the Rust workspace once per supported local system and exposes CLI app entrypoints.
  - What domain intent: makes Autopoietic core tools available as OS organs and developer commands.
  - Why: confident — ADR 0004 and ADR 0008 require Rust core tools to be first-class distribution components.
  - Invariants: package outputs must stay evaluable for all listed systems; x86-only ISO output must not force aarch64 evaluation.
  - Failure modes: referencing the x86 ISO unconditionally from all systems would break non-x86 flake package evaluation.
  - Connections ←: `Cargo.lock`, `nixpkgs`, `systems`, `self.packages`.
  - Connections →: dev shell, app outputs, NixOS ISO system packages.

- `nixosConfigurations.iso` in `flake.nix`
  - What mechanical: evaluates an x86_64 NixOS system from locked nixpkgs' minimal installer module, local ISO host config, and the Autopoietic core module.
  - What domain intent: creates a distributable live/install image boundary separate from the `aion` development host.
  - Why: confident — ADR 0007 requires a flake ISO build target and ADR 0008 requires ISO configuration to be separated from development host configuration.
  - Invariants: must import the locked nixpkgs installer module; must pass the locally built Autopoietic tools into the host config; must keep mutation mode observe-only.
  - Failure modes: omitting the installer module leaves no `system.build.isoImage`; omitting `specialArgs.autopoieticTools` leaves the host config unable to include the tools.
  - Connections ←: locked nixpkgs installer module, `self.nixosModules.autopoietic-core`, `self.packages.x86_64-linux.autopoietic-tools`.
  - Connections →: `packages.x86_64-linux.iso`, ISO derivation, live boot system closure.

- `hosts/iso/configuration.nix`
  - What mechanical: declares live ISO identity, enables flakes, includes Autopoietic tools plus a minimal editor, sets ISO edition, enables core module, and pins mutation runner to observe-only.
  - What domain intent: provides the smallest Autopoietic live environment that can inspect itself and installed targets without performing live mutation.
  - Why: confident — ADR 0007 says initial runtime must be observe-only; ADR 0008 allows Git, Rust core tools, systemd units, memory initialization, mutation workspace, and minimal editor/recovery tools.
  - Invariants: no GUI, no large AI runtime, no live mutation, no host-specific hardware config.
  - Failure modes: enabling agent runtime or mutate-live/autopoiesis here would violate ADR 0007/0008 safety constraints.
  - Connections ←: `autopoieticTools` special arg, core module options, locked installer defaults.
  - Connections →: `/etc/autopoietic/*.json`, tmpfiles memory directories, ISO package closure.

Hypothesis verdict: confirmed. The ISO configuration is an additive live/install media layer over the locked NixOS minimal installer, while `aion` remains the development/seed host.

Open questions: initial user and install workflow remain deferred to a future ADR; this change only creates an evaluate/build target.

Risk hotspots: the locked minimal installer module includes more recovery packages than the project-specific allowlist in ADR 0008; this is accepted for now as reuse of the upstream installer base, not as project-specific organ growth.

## External grounding

- External surface: locked nixpkgs NixOS minimal installer ISO modules and `system.build.isoImage` output shape.
- Stale risk: Class A / B — NixOS module names, image build options, and flake output conventions are version-sensitive and must match the pinned `flake.lock` nixpkgs revision.
- Local anchor:
  - `flake.lock` pins nixpkgs revision `549bd84d6279f9852cae6225e372cc67fb91a4c1`.
  - Local Nix is `2.34.7` as recorded in `docs/implementation/nix-flake-grounding.md`.
  - Locked source path: `/nix/store/h9wn92hv33sizprch7fcp16lfs1k3w5j-source`.

## Claims

### EG-001

- Claim: The locked nixpkgs minimal installer module is the correct import surface for a minimal non-graphical NixOS ISO.
- Source: `/nix/store/h9wn92hv33sizprch7fcp16lfs1k3w5j-source/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix`.
- Source quality: local locked nixpkgs source.
- Version fit: matches `flake.lock` nixpkgs revision.
- Executable probe: `nix eval --impure --raw --expr 'let flake = builtins.getFlake "path:/home/haru/OS"; in (flake.inputs.nixpkgs.lib.nixosSystem { system = "x86_64-linux"; modules = [ "${flake.inputs.nixpkgs}/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix" ]; }).config.system.build.isoImage.drvPath'` produced `/nix/store/lg5i4niipnhg405gpf5xkcqcgv8isyhb-nixos-minimal-26.05.20260505.549bd84-x86_64-linux.iso.drv`.
- Lateral check: locked `nixos/tests/boot.nix` evaluates `../modules/installer/cd-dvd/installation-cd-minimal.nix` and uses `.config.system.build.isoImage` for CD/USB boot tests.
- Decision: confirmed.

### EG-002

- Claim: `config.system.build.isoImage` is the derivation to expose as the flake ISO package output for this locked nixpkgs revision.
- Source: `/nix/store/h9wn92hv33sizprch7fcp16lfs1k3w5j-source/nixos/modules/installer/cd-dvd/iso-image.nix` lines 1037-1056.
- Source quality: local locked nixpkgs source.
- Version fit: matches `flake.lock` nixpkgs revision.
- Executable probe: `nix eval --impure --raw --expr 'let flake = builtins.getFlake "path:/home/haru/OS"; in (flake.inputs.nixpkgs.lib.nixosSystem { system = "x86_64-linux"; modules = [ "${flake.inputs.nixpkgs}/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix" ]; }).config.system.build.image.drvPath'` produced the same derivation path as `system.build.isoImage`.
- Lateral check: locked `nixos/release.nix` uses `(import lib/eval-config.nix { ... }).config.system.build.isoImage` for `makeIso`.
- Decision: confirmed.

### EG-003

- Claim: The local Autopoietic ISO configuration evaluates to an observe-only ISO derivation.
- Source: local `flake.nix` and `hosts/iso/configuration.nix`.
- Source quality: local project source plus executable Nix evaluation.
- Version fit: matches the locked flake inputs.
- Executable probe: `nix eval --raw .#nixosConfigurations.iso.config.system.build.isoImage.drvPath` produced `/nix/store/s8syqjax9421dq98g174ipr31nazi38x-nixos-autopoietic-26.05.20260505.549bd84-x86_64-linux.iso.drv`; `nix eval --json .#nixosConfigurations.iso.config.autopoietic.mutationRunner.mode` produced `"observe-only"`.
- Lateral check: `nix eval --json .#nixosConfigurations.iso.config.environment.systemPackages` includes `autopoietic-tools-0.1.0`, `nano-9.0`, and upstream installer packages including `git-2.53.0`.
- Decision: confirmed for evaluation and ISO build.

## Implementation constraints

- Use `(nixpkgs + "/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix")` from the locked flake input for the first ISO.
- Expose `self.nixosConfigurations.iso.config.system.build.isoImage` as `packages.x86_64-linux.iso` only; do not force ISO evaluation for every system in `systems`.
- Keep the initial ISO in `observe-only`; do not enable live mutation or a heavy agent runtime.
- Treat the broad upstream installer package set as inherited installer substrate, not as project-specific organ additions.

## Verification evidence

- `nix eval --raw .#nixosConfigurations.iso.config.system.build.isoImage.drvPath`
- `nix eval --json .#nixosConfigurations.iso.config.autopoietic.mutationRunner.mode`
- `nix eval --raw .#packages.x86_64-linux.iso.drvPath`
- `nix flake check --no-write-lock-file`
- `nix build .#packages.x86_64-linux.iso --no-link --print-out-paths` produced `/nix/store/vjcsf7mffqk38cx1m3203hi6qprr32v1-nixos-autopoietic-26.05.20260505.549bd84-x86_64-linux.iso`
