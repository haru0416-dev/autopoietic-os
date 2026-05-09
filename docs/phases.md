# Phases

This project uses `P` as the commit-level phase marker. A phase commit should represent a coherent system capability, not just a pile of files.

## Current phase: P0 baseline

P0 is the committed baseline. The repository contains focused BIOS and UEFI VM checks for an observe-only ISO, plus black-box BIOS and UEFI boot checks for the production ISO artifact. The verification boundary is defined in [ADR 0012](adr/0012-p0-iso-verification-boundary.md).

The next planned phase is P1: an offline mutation verifier. Its boundary is defined in [ADR 0013](adr/0013-p1-offline-mutation-verifier-boundary.md).

## P-1: bootstrap scaffold

P-1 is the bootstrap scaffold before the first baseline. It is allowed to contain incomplete organs when they are necessary to make the genome evaluable and the first ISO buildable.

P-1 includes:

- ADRs and protocols that define the genome, mutation, effect, grounding, semantic review, and Rust discipline rules;
- the flake-based NixOS seed;
- the `aion` development host configuration;
- core NixOS modules for identity, memory, observation, agent runtime placeholder, and mutation-runner mode;
- the Rust workspace with initial `os-introspect` and `mutation-journal` CLIs;
- memory schemas and JSONL ledgers;
- a minimal ISO host configuration;
- a flake ISO package output that can build an Autopoietic NixOS ISO artifact.

P-1 is complete when:

- the repository is initialized as a Git repository;
- generated build artifacts such as `target/` are ignored;
- `nix flake check --no-write-lock-file` passes;
- `nix build .#packages.x86_64-linux.iso --no-link --print-out-paths` produces an ISO store path;
- the current status is documented as pre-P0.

P-1 does not claim that the ISO boots or that the system can safely mutate itself.

## P0: observe-only ISO baseline

P0 is the first baseline suitable for phase-level commits after bootstrap. P0 is reached when the ISO is not merely buildable, but boot-verified as an observe-only system.

P0 requires:

- a VM boot check for the ISO;
- black-box VM boot checks for the production ISO artifact;
- proof that the booted system reaches `multi-user.target`;
- proof that `/etc/autopoietic/identity.json`, `/etc/autopoietic/observation.json`, and `/etc/autopoietic/mutation-runner.json` exist in the booted system;
- proof that the mutation runner mode is `observe-only`;
- proof that the memory root initializes mutation, effect, generation, and organ directories;
- proof that `os-introspect` runs inside the booted ISO;
- proof that `mutation-journal` can append to a non-persistent test ledger inside the booted ISO;
- a UEFI CD-ROM boot check for the instrumented ISO;
- documentation for building and smoke-testing the ISO;
- no live mutation, no `autopoiesis` mode, no heavy agent runtime, and no GUI by default.

P0 accepts the verification split from ADR 0012: detailed observe-only assertions run on a test-instrumented ISO built from the same configuration, while the production ISO artifact is validated by black-box serial-console boot to `multi-user.target` under BIOS and UEFI.

## After P0

Post-P0 phases should be proposed only after the previous phase has executable verification. Likely future phase themes are:

- P1: offline mutation verifier for draft patches;
- P2: VM-tested mutation promotion;
- P3: install workflow and generation lineage linking;
- P4: organ registry and decay review.

These names are placeholders until each phase has an ADR or implementation plan.

## P1: offline mutation verifier

P1 starts the verifier-guided mutation pipeline without allowing the system to mutate itself live. The scope is fixed by [ADR 0013](adr/0013-p1-offline-mutation-verifier-boundary.md).

P1 requires:

- a mutation proposal format with goal, phase, changed paths, expected checks, patch body, and side-effect declaration;
- a verifier CLI that applies a proposal only to a temporary worktree or copy;
- no live `nixos-rebuild switch` and no mutation applied to the running system;
- structured verification results for passed, rejected, and errored proposals;
- journal entries for both successful and failed verification attempts;
- tests for a valid proposal, a malformed patch, an undeclared side effect, and a failed check being recorded;
- documentation that P1 is still not autonomous mutation.

P1 does not include AI patch generation, live activation, automatic revert, generation lineage promotion, install workflow, GUI, or heavy agent runtime.
