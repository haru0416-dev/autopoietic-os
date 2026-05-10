# Evidence bundles and canonical comparison

ADR 0016 introduces Evidence Bundles as the cross-phase handoff unit for verification, promotion, install planning, and later generation acceptance. This note fixes the first mapping rules before any CLI emits bundles.

The goal is not to replace existing P1/P2/P3 records immediately. The first step is to make every future bundle say what was observed, what was normalized for comparison, and which claim is backed by which evidence.

## Data layers

Evidence data is split into three layers.

- Raw: bytes, logs, JSONL entries, schema files, command output, and target-root files as observed.
- Canonical: normalized facts used for comparison, with volatile fields excluded.
- Derived: claims made by a tool, policy, AI, or human.

Derived claims must not stand alone. A claim is usable by a later phase only when its `backing` points to observations, canonical facts, or comparison reports in the same bundle.

## Current vocabulary

The initial shared vocabulary lives in `autopoietic-core` and `memory/evidence-bundle.schema.json`. P1, P2, and the current P3 install-plan / install-verify outputs have pure Rust mapping functions into `EvidenceBundle`; optional CLI sidecar emission uses those mappings without changing the primary phase gates.

- `EvidenceBundle`: phase handoff envelope.
- `EvidenceSubject`: mutation/proposal/root or generation identity.
- `EvidenceInputRef`: inputs consumed by a phase.
- `EvidenceObservation`: raw observation with provenance.
- `CanonicalFact`: normalized fact for comparison.
- `ComparisonReport`: explicit comparison result.
- `EvidenceClaim`: derived conclusion with backing and limits.
- `ProvenanceRef` and `DigestRef`: content-addressed raw reference.
- `ComparisonStatus`: `matched`, `mismatched`, `missing`, `incomparable`, `stale`, `ambiguous`, `error`.
- `DataQuality`: `raw`, `observed`, `canonicalized`, `verified`, `derived`, `stale`, `ambiguous`, `unknown`.

## P1 verification mapping

A P1 verification bundle should use the `MutationVerificationRecord` as its first source record.

Subject:

- `mutation_id`: from the verification record.
- `proposal_fingerprint`: from the verification record.
- `root_fingerprint`: from the verification record.

Inputs:

- proposal manifest and patch source, each with a SHA-256 digest when available;
- copied root fingerprint as the canonical base-genome input;
- verifier configuration or check list when it affects the result.

Observations:

- patch application result;
- Nix check result;
- proposal `expected_checks` results;
- the raw verification record as a JSONL provenance reference.

Canonical facts:

- mutation ID;
- proposal fingerprint;
- root fingerprint;
- check names and statuses;
- changed paths.

Claims:

- `mutation verified`, backed by passed checks and matched proposal/root fingerprints;
- `mutation rejected`, backed by failed checks or invalid inputs;
- `verification errored`, backed by parser, filesystem, or command errors.

Limits:

- P1 does not boot a VM.
- P1 does not install, activate, or write to the live system.
- P1 `verified` is not P2 `promoted` and not generation `accepted`.

## P2 promotion mapping

A P2 promotion bundle should bind P1 evidence to VM promotion evidence.

Subject:

- `mutation_id`: from the promotion record.
- `proposal_fingerprint`: from the promotion record.
- `root_fingerprint`: use `promotion_root_fingerprint`.

Inputs:

- selected P1 verification record;
- current proposal manifest and patch source;
- parent genome reference;
- selected VM checks.

Comparisons:

- current proposal fingerprint vs P1 proposal fingerprint;
- current root fingerprint vs P1 root fingerprint;
- selected target configuration vs VM check name prefix;
- VM check status vs required passed status.

Claims:

- `mutation promoted`, backed by matched P1/P2 fingerprints and passed VM checks;
- `promotion rejected`, backed by mismatch, failed VM check, ambiguous selector, or incomplete P1 evidence;
- `promotion errored`, backed by command, copy, replay, or parse errors.

Limits:

- P2 does not accept the mutation into generation lineage.
- P2 does not write installed memory.
- P2 does not run live `nixos-rebuild switch`.

## P3 install planning and seed verification mapping

P3 has two evidence-producing shapes today: `install-plan` and `install-verify`.

For `install-plan`, the bundle binds a promoted P2 record to a planned generation and seed manifest.

Subject:

- `mutation_id`: from the seed manifest or generation record.
- `proposal_fingerprint`: from P2 metadata.
- `generation_id`: planned resulting generation.

Inputs:

- selected P2 promotion record;
- selected P1 verification record;
- target root path as declared input;
- parent and resulting generation labels.

Claims:

- `install planned`, backed by promoted P2 evidence, P1 verification seed match, and generated seed manifest hashes.

Limits:

- `install-plan` does not write to the target root.
- `lineage_status: planned` is not an installed generation.

For `install-verify`, the bundle compares the seed manifest against files currently visible under the target root.

Comparisons:

- expected seed file hash vs actual file hash;
- manifest target path vs `target_root + installed_path`;
- each file status as `matched`, `missing`, `mismatched`, or `error`.

Claims:

- `seed files verified`, backed by all file comparisons being `matched`;
- `seed verification failed`, backed by any missing, mismatched, or errored file.

Limits:

- `install-verify` checks seed-file hashes only.
- It does not evaluate the installed NixOS configuration.
- It does not prove that `nixos-install` ran.

## Volatile fields

Canonical comparisons should not depend on fields that naturally change between equivalent runs.

Treat these as volatile by default:

- timestamps such as `timestamp`, `generated_at`, `verified_at`, and `observed_at`;
- temporary worktree paths;
- runtime log paths;
- generated UUIDs when they are not the subject identity;
- stdout/stderr text when a digest or status is enough for the comparison;
- JSONL append order unless order is the behavior being checked;
- optional Evidence Bundle sidecars under `memory/evidence/`.

Keep volatile fields in raw provenance. Do not use them as the only reason for `mismatched` unless the phase explicitly checks freshness or ordering. Optional bundle sidecars must not change P1/P2 root fingerprint gates or copied-worktree check outcomes.

## AI-facing handoff

AI handoffs should compress evidence without severing provenance.

Use this shape:

```text
Summary:
Canonical facts:
Evidence refs:
Claims:
Limits:
Open uncertainty:
```

Rules:

- `Summary` is for reading convenience only.
- `Canonical facts` should be stable, normalized, and refer to bundle IDs or fact IDs.
- `Evidence refs` must include source and digest when available.
- `Claims` must name backing IDs.
- `Limits` must say what the claim does not prove.
- `Open uncertainty` should use `unknown`, `ambiguous`, `stale`, or `incomparable` rather than hiding uncertainty in prose.

## Implementation order

1. Keep the current P1/P2/P3 records as the primary executable records.
2. Add pure mapping functions from those records into `EvidenceBundle` — implemented for P1 verification and P2 promotion records.
3. Add P3 `install-plan` and `install-verify` mapping functions once their proposal fingerprint and generation identity inputs are explicit — implemented for the current P3 records.
4. Emit optional bundle files or JSON output only after the mappings are tested — implemented for P1 verification, P2 promotion, and current P3 install-plan / install-verify outputs as non-gating sidecars.
5. Use bundles as gates only after P1/P2/P3 behavior remains unchanged under the new evidence layer.
