# P1 mutation verifier

P1 adds the first executable slice of the verifier-guided mutation pipeline. It accepts a mutation proposal, copies the repository to an isolated worktree, applies the proposal patch there, runs verifier checks, and appends a structured result record.

It does not apply the patch to the live repository or run `nixos-rebuild switch`.

## Proposal shape

Proposals are JSON manifest documents matching `memory/mutation-proposal.schema.json`. The recommended authoring layout keeps the patch as a normal diff file next to the manifest:

```text
mutations/pending/<mutation-id>/
  proposal.json
  patch.diff
```

Required fields:

- `schema_version`: currently `0.1.0`
- `mutation_id`: stable ID for the proposed mutation
- `goal`: intent of the mutation
- `phase`: phase or test context, such as `P1`
- `changed_paths`: relative paths the patch is allowed to change
- `expected_checks`: targeted checks to run after the patch applies
- `patch_path`: proposal-directory-relative path to the unified diff file

Optional fields:

- `patch`: inline unified diff body, kept for small tests and compatibility; do not set it together with `patch_path`
- `side_effects`: declared non-Nix side effects, using the same risk vocabulary as the effect ledger
- `metadata`: string key/value annotations

`patch_path` is preferred over inline `patch` because normal diff files are easier for humans and agents to read, review, and edit without JSON string escaping.

## Verifier behavior

`mutation-runner verify --proposal <file>` performs these steps:

1. Parse the proposal JSON.
2. Read exactly one patch source: inline `patch` or proposal-relative `patch_path`.
3. Validate the proposal boundary: schema version, relative changed paths, and patch paths matching `changed_paths`.
4. Copy the root to a temporary worktree, skipping `.git`, `target`, `result*` outputs, JSONL ledgers, and optional `memory/evidence/` sidecars.
5. Apply the unified diff inside the temporary worktree with the built-in patch applier.
6. Run `nix flake check --no-write-lock-file path:<temporary-worktree>`.
7. Run proposal `expected_checks` inside the temporary worktree.
8. Append a verification result to `memory/mutation-results.jsonl`, or to `--journal <path>` if provided.
9. If `--evidence-bundle <path>` is supplied, attempt to write a derived `EvidenceBundle` JSON file for the verification record.

The CLI exits successfully only when the proposal is `verified`. Rejected and errored proposals are still journaled before the CLI returns a non-zero exit. Evidence bundle output is optional and does not replace or gate the JSONL verification journal; invalid or unwritable bundle output is skipped with a warning after preserving the primary record.

Verification records include first-class `proposal_fingerprint` and `root_fingerprint` fields. Both use `sha256:<hex>` when the evidence can be computed. The root fingerprint covers the copied genome content, excluding proposal inputs, build outputs, `.git`, JSONL ledgers, and optional `memory/evidence/` sidecars. P2 promotion uses the proposal fingerprint to reject proposal content that changed after P1 verification, and uses the root fingerprint to reject promotion from a different base genome.

## Side-effect boundary

P1 is intentionally conservative. The verifier runs its own `nix flake check` for the isolated worktree, and proposal checks are limited to `test`, `true`, and `false` with paths that stay inside the worktree. Other commands are rejected in P1, even when the proposal declares side effects.

Even with a declaration, P1 still applies patches only inside the isolated worktree. Live side effects and live activation are P2-or-later concerns.

## Verification evidence

The behavior tests are in `crates/mutation-runner/src/verifier.rs` and cover:

- valid docs proposal verifies and is journaled;
- proposal-relative `patch_path` verifies and is journaled;
- unsafe or ambiguous patch sources are rejected or errored before worktree mutation;
- malformed patch is rejected and journaled;
- undeclared and declared side-effect commands are rejected;
- allowlisted checks with absolute host paths are rejected;
- failed check is rejected and journaled;
- work directories inside the live root, including symlinked paths, are rejected;
- deletion patches are supported, while malformed partial deletions are rejected.

Run them with:

```bash
cargo test -p mutation-runner
```
