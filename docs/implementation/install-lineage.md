# Install planning and generation lineage

P3 begins with a dry-run install-plan boundary. The goal is to connect P2 promotion evidence to a generation lineage record before any target root or disk mutation is possible.

## Scope

`mutation-runner install-plan` reads promotion evidence from `memory/mutation-promotions.jsonl` and P1 verification evidence from `memory/mutation-results.jsonl` by default. It selects a promotion record by `--promotion-id` or `--mutation-id`. If `--mutation-id` matches more than one promotion record, the command rejects the plan and requires `--promotion-id` to avoid binding generation lineage to the wrong evidence.

The selected record must be `promoted` and must include non-empty passing VM check evidence. Rejected, errored, incomplete, or non-passing promotion evidence cannot enter P3 generation lineage.

The command requires:

- an explicit absolute `--target-root`;
- `--parent-generation`;
- `--resulting-generation`.

By default, the command prints an install-plan JSON object containing a `generation` record and a `seed_manifest`. It does not write any target-root file. Passing `--record` appends only the `generation` record to `memory/generations.jsonl`. Passing `--evidence-bundle <path>` attempts to write a derived, non-gating P3 EvidenceBundle sidecar. The generation journal path must not be inside the install target root and must not traverse symlinks when `--record` is used.

## Evidence carried forward

The generated lineage record includes first-class P3 fields for:

- `lineage_status`, set to `planned` for install-plan output;
- `verification_id`;
- `promotion_id`;
- `target_root`;
- `target_configuration`.

It also carries P2 fingerprints in metadata:

- `parent_genome`;
- `proposal_fingerprint`;
- `verified_root_fingerprint`;
- `promotion_root_fingerprint`.

## Seed manifest

The seed manifest lists the target-root files a later install execution would write. Each file entry includes:

- `installed_path`, the logical path inside the installed system;
- `target_path`, the path under the explicit `--target-root`;
- `source`, the evidence or synthetic seed source;
- `content_sha256`, a digest of the planned JSON content;
- `effect`, the planned effect-ledger shape for that future write.

The initial manifest covers identity, P1 verification record, P2 promotion evidence, generation lineage, and planned effect ledger seed files. Ledger seed hashes are computed from JSONL entries that match the corresponding local Rust record types. It is a plan only; no listed `target_path` is created by `install-plan`.

## Safety boundary

This initial P3 slice does not run `nixos-install`, does not call `nixos-rebuild`, does not partition disks, and does not write to the target root. The only optional write is the local generation journal append under `--record`.

Real install execution, installed memory seeding, effect ledger writes for target-root side effects, and post-install NixOS evaluation are deferred until their external command behavior is grounded and the user explicitly approves those side effects. Later install execution must transition lineage from `planned` to `installed` or `failed`; planned records must not be treated as accepted installed generations.

## Read-only seed verification

`mutation-runner install-verify --plan <plan.json>` reads an install-plan output and verifies the target files listed in `seed_manifest.files` without writing anything. Passing `--evidence-bundle <path>` attempts to write a derived, non-gating EvidenceBundle sidecar for the read-only verification result. The verifier treats each seed entry as a regular file whose `target_path` must equal `target_root` plus the entry's absolute `installed_path`. Each file is reported as:

- `matched`, when the target file exists and its SHA-256 matches `content_sha256`;
- `missing`, when the target file is absent;
- `mismatched`, when the target file exists but has different content;
- `error`, when the verifier cannot read the target path.

The command exits successfully only when all listed files are `matched`. Missing, mismatched, unreadable, non-regular, or symlink-traversing seed files produce a JSON report and a non-zero exit. Invalid manifests are rejected before report generation. The verifier rejects manifests with no files, unsupported seed schema versions, malformed seed hashes, relative or parent-traversing installed paths, target paths that are relative or outside the manifest `target_root`, and target paths that do not match `target_root` plus `installed_path`.

This verifier checks only the planned seed-file hashes. Installed-root NixOS evaluation and actual install execution remain deferred until their external command behavior is grounded.
