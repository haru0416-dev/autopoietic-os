# Rust workspace grounding — 2026-05-08

This note records the external-grounding constraints used for the first Rust workspace implementation.

## External grounding

- External surface: Rust toolchain and crate APIs used by `autopoietic-core`, `os-introspect`, `mutation-journal`, and `mutation-runner`.
- Stale risk: Class A / B — crate APIs and resolved versions may differ from remembered examples or unconstrained latest documentation.
- Local anchor:
  - `rustc 1.94.1 (e408947bf 2026-03-25)` from `rtk rustc --version`.
  - `cargo 1.94.1 (29ea6fb6a 2026-03-24)` from `rtk cargo --version`.
  - `Cargo.lock` resolved versions:
    - `anyhow 1.0.102`
    - `chrono 0.4.44`
    - `clap 4.6.1`
    - `serde 1.0.228`
    - `serde_json 1.0.149`
    - `uuid 1.23.1`

## Claims

### EG-001

- Claim: The workspace can use Rust 2024 edition with resolver `3` on the local toolchain.
- Source: local toolchain probes and `Cargo.toml`.
- Source quality: local anchor.
- Version fit: matches local `rustc` / `cargo` 1.94.1.
- Executable probe: `rtk cargo check --workspace` passed.
- Lateral check: `rtk cargo fmt --check` and `rtk cargo clippy --workspace --all-targets -- -D warnings` also processed the workspace successfully.
- Decision: confirmed.

### EG-002

- Claim: The selected `clap`, `serde`, `serde_json`, `chrono`, `uuid`, and `anyhow` APIs used by the CLIs compile with the locally resolved crate versions.
- Source: `Cargo.lock` and local cargo registry source under `/home/haru/.cargo/registry/src/`.
- Source quality: local anchor plus local resolved source.
- Version fit: matches `Cargo.lock`.
- Executable probe: `rtk cargo check --workspace` passed.
- Lateral check: `rtk cargo clippy --workspace --all-targets -- -D warnings` passed against the same resolved crate graph.
- Decision: confirmed for the API surface actually used in this implementation.

### EG-003

- Claim: The current `flake.nix` Rust packaging stanza is buildable with this environment's Nix.
- Source: `flake.nix`.
- Source quality: local project source.
- Version fit: updated by `docs/implementation/nix-flake-grounding.md` after Nix became available.
- Executable probe: `nix build .#autopoietic-tools --no-link --print-out-paths` succeeded.
- Lateral check: `nix flake check` evaluated the package derivation successfully.
- Decision: confirmed; see `docs/implementation/nix-flake-grounding.md` for the current Nix-side evidence.

## Implementation constraints

- Use the APIs that passed `cargo check` and `clippy` under the resolved `Cargo.lock`.
- The Nix packaging stanza is now confirmed by the Nix-side probes recorded in `docs/implementation/nix-flake-grounding.md`.
- Keep CLI behavior testable without Nix; `os-introspect` must degrade to partial observations when Nix commands are unavailable.

## Verification needed during execution-loop

- Re-run `rtk cargo check --workspace` after Rust changes.
- Re-run `rtk cargo clippy --workspace --all-targets -- -D warnings` before handoff.
- Re-run `nix flake check` and `nix build .#autopoietic-tools --no-link` after flake or packaging changes.
