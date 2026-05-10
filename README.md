# Autopoietic OS

Autopoietic OS is an experimental NixOS seed for treating the system configuration as a genome: AI proposes mutations as Nix patches, verifier loops test them, and accepted generations become an evolutionary lineage.

The long-term target is a complete, minimal NixOS-based distribution that can be distributed as an ISO image, not merely a set of tools installed on an existing machine. Existing NixOS components are reused when they fit; when they are too large or imprecise, they are distilled or replaced with purpose-built organs.

This repository currently implements the first development slice:

- a flake-based NixOS seed;
- Home Manager integration;
- core NixOS modules for identity, memory, observation, agent runtime, and mutation running;
- Rust workspace CLIs, including `os-introspect`, `mutation-journal`, and `mutation-runner`;
- schemas and phase design documents;
- a boot-verified observe-only ISO baseline with VM checks;
- a P1 offline mutation verifier for draft patch proposals;
- a P2 VM-tested promotion gate for verified mutations;
- an initial P3 install-plan entrypoint that links promoted mutations to generation lineage records without running a live install.

## Initial contract

The first milestone is not autonomous mutation. It is self-observation: the OS should be able to read itself as Nix structure and produce machine-readable state for later patch synthesis.

The current repository state is the **initial P3 install-plan and generation lineage link**: mutation proposals can be verified in an isolated worktree, replayed into an isolated promotion worktree for NixOS VM checks, then converted into a dry-run install plan and generation lineage record. The P0 ISO baseline remains boot-verified with test-instrumented detailed checks and production BIOS/UEFI black-box boot checks. See [`docs/phases.md`](docs/phases.md), [ADR 0012](docs/adr/0012-p0-iso-verification-boundary.md), [ADR 0013](docs/adr/0013-p1-offline-mutation-verifier-boundary.md), [ADR 0015](docs/adr/0015-p2-vm-tested-mutation-promotion-boundary.md), and [ADR 0014](docs/adr/0014-p3-install-workflow-and-generation-lineage-boundary.md).

## Quick start

```bash
nix develop
os-introspect --root . --output memory/self-state.json
mutation-journal append --goal "bootstrap self-observation" --status accepted --phase scaffold
mutation-runner verify --proposal path/to/proposal.json --evidence-bundle memory/evidence/p1.json
mutation-runner promote --proposal path/to/proposal.json --parent-genome git:<revision> --evidence-bundle memory/evidence/p2.json
mutation-runner install-plan --mutation-id mut-example --target-root /mnt/autopoietic --parent-generation gen-parent --resulting-generation gen-child
mutation-runner install-verify --plan path/to/install-plan.json
```

For authored mutations, prefer `proposal.json` plus a sibling `patch.diff`; inline JSON patch strings are supported mainly for small tests and compatibility.
Promotion reads P1 verification evidence from `memory/mutation-results.jsonl` by default and appends P2 promotion evidence to `memory/mutation-promotions.jsonl`.
Install planning reads P2 promotion evidence and P1 verification evidence from `memory/` journals by default. It prints a dry-run install-plan object with `lineage_status: planned` and a seed manifest unless `--record` is passed; it does not run `nixos-install` or mutate a target root. Install verification reads an install plan and checks listed seed-file hashes without writing to the target root.

## ISO smoke test

Build and boot-test the Autopoietic ISO configuration with the NixOS VM test driver:

```bash
nix build .#checks.x86_64-linux.iso-boot-basic --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-observe-only --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-tools --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-uefi-boot --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-production-boot-console --no-link --print-out-paths
nix build .#checks.x86_64-linux.iso-production-uefi-boot-console --no-link --print-out-paths
```

In an uncommitted worktree, use the explicit path form so Nix sees untracked files:

```bash
nix build path:/home/haru/OS#checks.x86_64-linux.iso-boot-basic --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-observe-only --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-tools --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-uefi-boot --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-production-boot-console --no-link --print-out-paths
nix build path:/home/haru/OS#checks.x86_64-linux.iso-production-uefi-boot-console --no-link --print-out-paths
```

`checks.x86_64-linux.iso-boot` is kept as an alias for the basic BIOS boot check.

## CI

GitHub Actions workflows are split into fast Rust/schema/Nix package checks and KVM-backed VM checks. See [`docs/ci.md`](docs/ci.md).

## Layout

```text
hosts/       host-level NixOS configurations
modules/     core modules and future organs
home/        Home Manager configuration
crates/      Rust workspace for core CLIs and libraries
memory/      schemas plus JSONL ledgers
mutations/   pending, accepted, failed, and reverted patches
tests/       VM and organ checks
docs/        manifesto, ontology, protocols, anti-goals
```

## Architecture decisions

Design decisions are recorded as ADRs under [`docs/adr/`](docs/adr/README.md). Start there before extending the implementation; the ADRs define the boundaries between Nix mutation, side effects, Rust tooling, observation, and verifier feedback.

## Safety model

Nix generations only roll back declarative system configuration. Anything outside that boundary, such as file writes or external API calls, must be recorded in the effect ledger with reversibility and compensation metadata.
