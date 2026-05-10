# P2 mutation promotion

P2 adds the VM-tested promotion gate after P1 offline verification. It does not accept a mutation into a real generation and does not run `nixos-rebuild switch`.

## Promotion shape

Promotion consumes three inputs:

- a mutation proposal, using the same `proposal.json` plus `patch.diff` shape as P1;
- a P1 verification journal, `memory/mutation-results.jsonl` by default;
- an explicit parent genome reference, passed as `--parent-genome`.

Promotion writes structured records matching `memory/mutation-promotion.schema.json`. By default, entries are appended to `memory/mutation-promotions.jsonl`.

The root fingerprint covers the copied genome content, excluding proposal inputs, build outputs, `.git`, JSONL ledgers, and optional `memory/evidence/` sidecars. The isolated worktree copy uses the same JSONL and `memory/evidence/` exclusions so volatile evidence cannot silently differ between P1 and P2 while still influencing promotion checks.

## CLI behavior

`mutation-runner promote --proposal <file> --parent-genome <revision-or-digest>` performs these steps:

1. Parse the proposal JSON.
2. Find the latest P1 verification record for the proposal's mutation ID.
3. Reject the promotion unless that verification record is `verified`.
4. Read and validate exactly one proposal patch source.
5. Reject the promotion unless the current proposal fingerprint matches the P1 verification fingerprint.
6. Reject the promotion unless the current root fingerprint matches the P1 verification root fingerprint.
7. Reject VM checks whose names do not match the selected target configuration prefix.
8. Copy the root to a temporary promotion worktree.
9. Replay the patch inside that isolated worktree.
10. Run generated VM promotion checks with `nix build --no-link --print-out-paths --no-write-lock-file path:<worktree>#checks.<system>.<vm-check>`.
11. Append a promotion result to `memory/mutation-promotions.jsonl`, or to `--journal <path>` if provided.
12. If `--evidence-bundle <path>` is supplied, attempt to write a derived `EvidenceBundle` JSON file for the promotion record.

The default VM check is `iso-boot-basic`. Pass `--vm-check <name>` one or more times to select different flake checks. The default system is `x86_64-linux`, and the default target configuration label is `iso`. A selected VM check must begin with `<target-configuration>-`, so a record cannot claim target `aion` while running an `iso-*` check.

The CLI exits successfully only when the proposal is `promoted`. Rejected and errored promotions are still journaled before the CLI returns a non-zero exit. Evidence bundle output is optional and does not replace or gate the JSONL promotion journal; invalid or unwritable bundle output is skipped with a warning after preserving the primary record.

## Promotion evidence

Promotion records include:

- mutation ID, goal, phase, and changed paths;
- P1 verification evidence reference;
- proposal fingerprint, stored as a first-class field to bind P2 to the same proposal content P1 verified;
- verified root fingerprint and promotion root fingerprint, stored as first-class fields to bind P2 to the same base genome P1 checked;
- parent genome reference;
- target configuration label;
- changed organs, when supplied with `--changed-organ`;
- promotion checks and their stdout, stderr, exit code, and status.

P2 keeps the boundary between `verified`, `promoted`, and `accepted` explicit. A `promoted` mutation has passed the VM promotion gate, but it is not accepted into generation lineage until a later phase records that decision.

## External grounding note

External grounding
- External surface: Nix 2.34.7 `nix build` CLI options used by generated promotion checks.
- Stale risk: Class A / CLI churn, because `nix build` is still marked experimental by Nix and its interface may change.
- Local anchor: `nix --version` reported `nix (Nix) 2.34.7`.

Claims
- EG-001:
  - Claim: local `nix build` supports `--no-link`, `--print-out-paths`, and `--no-write-lock-file` for flake installables.
  - Source: local `nix build --help`; Nix 2.34.7 reference manual at `https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-build`.
  - Source quality: local CLI help plus version-matched official manual.
  - Version fit: matched to local Nix 2.34.7.
  - Executable probe: `nix build --dry-run --no-link --print-out-paths --no-write-lock-file path:/home/haru/OS#checks.x86_64-linux.iso-boot-basic` evaluated the default promotion command shape.
  - Lateral check: local CLI help and official Nix 2.34.7 manual both list the required flags.
  - Decision: confirmed.
- EG-002:
  - Claim: `sha2` 0.10.9 exposes `Sha256` and the `Digest` trait for SHA-256 hashing.
  - Source: docs.rs `sha2` 0.10.9 crate documentation; local Cargo resolution after adding `sha2 = "0.10.9"`.
  - Source quality: versioned crate documentation plus local Cargo resolution.
  - Version fit: matched to the locally resolved `sha2` 0.10.9 dependency.
  - Executable probe: `cargo test -p mutation-runner` compiled and exercised SHA-256 proposal/root fingerprinting.
  - Lateral check: docs.rs and Cargo's local resolver agree on the crate/version/API surface used by the implementation.
  - Decision: confirmed.

Implementation constraints
- Use: generated `nix build --no-link --print-out-paths --no-write-lock-file path:<worktree>#checks.<system>.<vm-check>` commands.
- Use: first-class `sha256:<hex>` proposal and root fingerprints in verification and promotion evidence.
- Do not use: `nixos-rebuild switch`, Nix profiles, or result symlinks in P2 promotion.
- Verification needed during execution-loop: dry-run or build the selected flake check target after changing promotion command generation.
- Verification needed during execution-loop: rerun mutation-runner tests after changing fingerprint inputs or schema fields.

## Test evidence

The behavior tests are in `crates/mutation-runner/src/promoter.rs` and cover:

- verified P1 evidence is promoted and journaled;
- missing verification evidence is rejected and journaled;
- rejected P1 evidence is not promoted;
- changed proposal content after P1 verification is rejected;
- changed root content after P1 verification is rejected;
- mismatched target configuration and VM check names are rejected;
- failed promotion checks are rejected and journaled;
- live root files are not changed by promotion replay.

Run them with:

```bash
cargo test -p mutation-runner
```
