# CI

This repository separates fast CI from VM-backed promotion evidence.

## Workflows

- `.github/workflows/ci.yml`
  - Rust formatting, check, clippy, and tests.
  - JSON schema and JSONL parse checks.
  - Nix build for `packages.x86_64-linux.autopoietic-tools`.
- `.github/workflows/vm.yml`
  - KVM preflight.
  - Nix installation with `system-features = nixos-test benchmark big-parallel kvm`.
  - Default P2 VM check: `checks.x86_64-linux.iso-boot-basic`.
  - Manual `workflow_dispatch` can run a space-separated list of VM checks or full `nix flake check`.

VM checks intentionally fail early after Nix/KVM setup when `/dev/kvm` is missing or remains inaccessible. That is better than silently treating a non-VM runner as promotion evidence.

## External grounding

External surface: GitHub Actions workflow syntax, `actions/checkout`, `cachix/install-nix-action`, GitHub-hosted Ubuntu runner tooling, and NixOS VM test KVM configuration.

Stale risk: Class A / versioned action and runner image churn. GitHub Actions action major versions, runner images, and installed toolchains change over time. KVM availability is runner-dependent.

Local anchor:

- Existing local Nix CLI: `nix (Nix) 2.34.7`.
- Existing repository checks: `cargo fmt --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `nix build --no-link --print-out-paths --no-write-lock-file path:/home/haru/OS#packages.x86_64-linux.autopoietic-tools` have been used locally.
- Local VM check failure without KVM produced `Required features: {kvm, nixos-test}`.

Claims:

- EG-CI-001:
  - Claim: GitHub workflow files belong under `.github/workflows` and support `pull_request`, `push`, `workflow_dispatch`, job permissions, and concurrency keys.
  - Source: GitHub Actions workflow syntax documentation.
  - Source quality: official GitHub documentation.
  - Version fit: applies to current GitHub Actions workflows.
  - Executable probe: workflow files are YAML-parsed locally as syntax files. GitHub-side workflow loading remains the required runtime probe.
  - Lateral check: GitHub workflow syntax documentation and repository path convention agree.
  - Decision: confirmed for file placement and static YAML shape; runtime loading remains inconclusive until GitHub runs the workflow.
- EG-CI-002:
  - Claim: `actions/checkout@v6` checks out the repository and recommends `contents: read` permission; `persist-credentials: false` is supported.
  - Source: `actions/checkout` README.
  - Source quality: official action repository documentation.
  - Version fit: `v6` is the current documented major version.
  - Executable probe: local YAML parsing confirms the step shape; final action execution happens when GitHub runs the workflow.
  - Lateral check: the same README documents usage and recommended permissions.
  - Decision: confirmed for documented workflow authoring; runtime action behavior remains inconclusive until GitHub CI runs.
- EG-CI-003:
  - Claim: `cachix/install-nix-action@v31` installs Nix for GitHub Actions, supports flakes, `github_access_token`, `enable_kvm`, and `extra_nix_config`, and documents NixOS test KVM setup using `system-features = nixos-test benchmark big-parallel kvm`.
  - Source: `cachix/install-nix-action` README.
  - Source quality: maintained action repository documentation.
  - Version fit: `v31` is the documented major version, with latest release in that series visible from the repository page.
  - Executable probe: local Nix commands confirm the command shape; final action behavior is probed by GitHub CI.
  - Lateral check: local Nix failure showed missing `{kvm, nixos-test}` features, matching the action documentation's KVM/system-features guidance.
  - Decision: confirmed for Nix command shape and KVM feature requirement; action runtime behavior remains inconclusive until GitHub CI runs.
- EG-CI-004:
  - Claim: `ubuntu-24.04` GitHub-hosted runners provide Rust, Cargo, Rustup, Rustfmt, Python, and sudo-capable Linux jobs.
  - Source: GitHub-hosted runners reference and Ubuntu 24.04 runner image README.
  - Source quality: official GitHub documentation and official runner image inventory.
  - Version fit: `ubuntu-24.04` is an explicit runner label, avoiding `ubuntu-latest` drift.
  - Executable probe: local Rust commands pass; GitHub runner availability is probed by workflow runtime.
  - Lateral check: GitHub-hosted runner reference and runner image README agree on Ubuntu runner availability and installed tool classes.
  - Decision: confirmed for selecting `ubuntu-24.04`; exact runner contents remain runtime-confirmed by GitHub CI.

Implementation constraints:

- Use explicit `ubuntu-24.04`, not `ubuntu-latest`, to reduce runner image drift.
- Use `permissions: contents: read` and `persist-credentials: false`; CI must not push.
- Keep fast checks separate from KVM-backed VM checks.
- VM workflow must fail early when `/dev/kvm` is missing or remains inaccessible after KVM setup.
- VM evidence should come from selected `checks.x86_64-linux.*` builds or full `nix flake check` on a KVM-capable runner.
- Treat the first GitHub run of each workflow as the runtime probe for action compatibility and KVM availability.
