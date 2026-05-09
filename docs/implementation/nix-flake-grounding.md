# Nix flake grounding — 2026-05-08

This note records the external-grounding constraints used to make the repository's Nix flake usable after Nix became available in the local environment.

## External grounding

- External surface: local Nix CLI, flake evaluation, NixOS configuration assertions, and `rustPlatform.buildRustPackage` package build.
- Stale risk: Class A / B — Nix CLI and NixOS module behavior are version-sensitive, and unlocked flakes may resolve to current upstream revisions.
- Local anchor:
  - `nix (Nix) 2.34.7` from `nix --version`.
  - `nix store info` reports daemon store version `2.34.7`.
  - `flake.lock` pins:
    - `nixpkgs` revision `549bd84d6279f9852cae6225e372cc67fb91a4c1`.
    - `home-manager` revision `fdb2ccba9d5e1238d32e0c4a3ec1a277efa80c1d`.

## Claims

### EG-001

- Claim: The local Nix installation supports flakes for this repository.
- Source: local CLI probe and Nix manual flake help output.
- Source quality: local executable anchor plus installed CLI help.
- Version fit: matches local Nix `2.34.7`.
- Executable probe: `nix flake --help` succeeded and listed `nix flake check`, `lock`, `show`, and related subcommands.
- Lateral check: `nix eval --expr '1 + 1'` and `nix store info` succeeded against the daemon store.
- Decision: confirmed.

### EG-002

- Claim: The seed host can be made flake-checkable with an evaluation-only root filesystem and disabled GRUB bootloader.
- Source: local nixpkgs module source and local flake check.
- Source quality: local resolved source from the locked nixpkgs revision.
- Version fit: matches `flake.lock` nixpkgs revision.
- Executable probe: `nix flake check --no-write-lock-file` passed after setting `fileSystems."/"` and `boot.loader.grub.enable = false` in the placeholder hardware config.
- Lateral check: local nixpkgs source includes a minimal evaluation config using `boot.loader.grub.enable = false`, `fileSystems."/".device = "nodev"`, and `fileSystems."/".fsType = "none"`; the NixOS assertion source also shows the previous failures were specifically missing root filesystem and GRUB boot configuration.
- Decision: confirmed for evaluation/checking only. This is not a real hardware configuration.

### EG-003

- Claim: `.#autopoietic-tools` builds from the locked flake.
- Source: local `flake.nix`, `Cargo.lock`, and locked nixpkgs.
- Source quality: local project source plus local executable build.
- Version fit: matches `flake.lock` and `Cargo.lock`.
- Executable probe: `nix build .#autopoietic-tools --no-link --print-out-paths` succeeded.
- Lateral check: `nix flake check` evaluated `packages.x86_64-linux.autopoietic-tools` and `packages.x86_64-linux.default` successfully.
- Decision: confirmed.

## Implementation constraints

- Keep `hosts/aion/hardware-configuration.nix` explicitly marked as evaluation-only until it is replaced by real `nixos-generate-config` output.
- Do not treat the placeholder host as installable hardware configuration.
- Keep `flake.lock` checked in as part of the genome so introspection can report pinned inputs.
- Use `nix flake check` and `nix build .#autopoietic-tools --no-link` as the Nix-side verification gate for this stage.

## Verification evidence

- `nix flake check` — passed.
- `nix build .#autopoietic-tools --no-link --print-out-paths` — produced `/nix/store/67928fsvv49wpqbxz26gbiq3hcgzyxyc-autopoietic-tools-0.1.0`.
- `nix run .#os-introspect -- --root . --output /tmp/opencode/nix-run-self-state.json` — produced self-state JSON with `has_lock: true` and inputs `home-manager`, `nixpkgs`.
- `nix run .#mutation-journal -- append ...` — appended and printed a mutation record.
