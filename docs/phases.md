# Phases

This project uses `P` as the commit-level phase marker. A phase commit should represent a coherent system capability, not just a pile of files.

## Current phase: initial P3 install planning and generation lineage

P3 is the current capability boundary. The repository contains a Rust `mutation-runner` CLI that verifies mutation proposals in an isolated worktree, promotes P1-verified mutations through an isolated VM-check gate, then creates dry-run install plans and generation lineage records from P2 promotion evidence without running a live install. The P3 boundary is defined in [ADR 0014](adr/0014-p3-install-workflow-and-generation-lineage-boundary.md).

P0 remains the committed ISO baseline. It contains focused BIOS and UEFI VM checks for an observe-only ISO, plus black-box BIOS and UEFI boot checks for the production ISO artifact. The P0 verification boundary is defined in [ADR 0012](adr/0012-p0-iso-verification-boundary.md).

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
- a flake ISO package output that can build an Autopoietic OS ISO artifact.

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

P1, P2, and P3 now have ADR boundaries. P3 is the next planned capability after P2 verification evidence is reviewed. Cross-phase evidence handoff is fixed by [ADR 0016](adr/0016-evidence-bundle-and-canonical-comparison-boundary.md); its initial shared vocabulary is represented by `EvidenceBundle` and `memory/evidence-bundle.schema.json`, with mapping rules in [evidence-bundles.md](implementation/evidence-bundles.md).

## P1: offline mutation verifier

P1 starts the verifier-guided mutation pipeline without allowing the system to mutate itself live. The scope is fixed by [ADR 0013](adr/0013-p1-offline-mutation-verifier-boundary.md).

P1 requires:

- a mutation proposal format with goal, phase, changed paths, expected checks, patch reference or inline body, and side-effect declaration — implemented by `MutationProposal` and `memory/mutation-proposal.schema.json`;
- a verifier CLI that applies a proposal only to a temporary worktree or copy — implemented by `mutation-runner verify`;
- no live `nixos-rebuild switch` and no mutation applied to the running system — enforced by isolated copy verification;
- structured verification results for passed, rejected, and errored proposals — implemented by `MutationVerificationRecord` and `memory/mutation-result.schema.json`;
- journal entries for both successful and failed verification attempts — written to `memory/mutation-results.jsonl` by default;
- tests for valid inline and `patch_path` proposals, malformed patches, invalid patch sources, side-effect boundary violations, and failed checks being recorded — covered by `cargo test -p mutation-runner`;
- documentation that P1 is still not autonomous mutation — documented here and in `docs/implementation/mutation-verifier.md`.

P1 does not include AI patch generation, live activation, automatic revert, generation lineage promotion, install workflow, GUI, or heavy agent runtime.

## P2: VM-tested mutation promotion

P2 promotes a P1-verified mutation into a VM-tested candidate without accepting it into a real generation or mutating the live system. Its scope is fixed by [ADR 0015](adr/0015-p2-vm-tested-mutation-promotion-boundary.md).

P2 requires:

- a promotion entrypoint that accepts only P1 `verified` mutation evidence plus matching proposal and root fingerprints — implemented by `mutation-runner promote`;
- replay or reproduction of the mutation in an isolated candidate worktree or copy — implemented by promotion replay before checks;
- NixOS build plus VM boot checks for the candidate target configuration — implemented as generated `nix build` checks for flake VM check outputs;
- phase-specific smoke assertions after VM boot — currently provided by the selected VM check derivations;
- structured promotion results for `promoted`, `rejected`, and `error` outcomes — implemented by `MutationPromotionRecord` and `memory/mutation-promotion.schema.json`;
- a promotion result journal that records successful and failed VM promotion attempts — written to `memory/mutation-promotions.jsonl` by default;
- generation lineage evidence for P3, including mutation ID, P1 verification reference, P2 promotion reference, proposal fingerprint, verified and promotion root fingerprints, parent genome revision or digest, candidate target configuration, changed paths or organs, and executed VM checks — recorded in promotion entries.

P2 does not include AI patch generation, live activation, install workflow, automatic revert, generation lineage acceptance, GUI, or remote/cloud promotion.

## P3: install workflow and generation lineage linking

P3 connects a VM-promoted mutation to an installed system generation. Its scope is fixed by [ADR 0014](adr/0014-p3-install-workflow-and-generation-lineage-boundary.md).

P3 requires P2 to exist first. P2 must define how a P1 `verified` mutation becomes VM-tested and eligible for promotion. P3 does not bypass that gate.

P3 requires:

- a minimal install workflow with explicit target root selection — initially implemented by `mutation-runner install-plan --target-root <absolute-path>`;
- dry-run or plan output before any install-side effect — implemented by default dry-run JSON output from `mutation-runner install-plan`;
- no destructive disk or live-system operation without explicit approval;
- generation lineage records linking mutation ID, parent generation, resulting generation, activation or install result, changed organs, lineage status, and verifier evidence — implemented for planned installs by `GenerationRecord` fields and optional `--record` journal writes;
- installed-system Autopoietic memory seed for identity, mutation results, generation ledger, and effect ledger — initially represented as a dry-run seed manifest in `mutation-runner install-plan` output;
- post-install verification that the installed root can be evaluated and that lineage entries are readable — seed-file hash verification is initially implemented by read-only `mutation-runner install-verify`;
- effect ledger entries for non-Nix side effects caused by install workflow steps.

The initial P3 slice intentionally stops before `nixos-install`, target-root writes, partitioning, or installed-root evaluation. Those require separate external grounding and explicit approval because they cross into live install side effects.

P3 does not include AI patch generation, live autonomous mutation, automatic revert, organ registry promotion, full installer UX, partitioning wizard, GUI, or remote/cloud install.
