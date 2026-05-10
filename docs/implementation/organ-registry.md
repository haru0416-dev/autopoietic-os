# Organ registry and decay review

P4 starts with a local, explicit organ registry. The goal is to record why reusable parts exist and to review decay without changing the live system.

## Registry records

Organ records use `OrganRecord` and `memory/organ.schema.json`. By default, `mutation-journal organ add` appends JSONL entries to the tracked default registry `memory/organs.jsonl`.

Required fields:

- `name`
- `type`
- `source`
- `purpose`

Optional fields:

- `created_by`
- `usage_count`
- `failure_count`
- `related_goals`
- `decay_status`

The supported `decay_status` values are `active`, `candidate`, `stale`, `duplicate`, and `failed`.

## CLI behavior

Register an organ:

```bash
mutation-journal organ add \
  --name mutation-journal \
  --type cli \
  --source crates/mutation-journal \
  --purpose "append Autopoietic OS memory records" \
  --created-by manual \
  --related-goal p4-organ-registry \
  --decay-status active
```

Read the registry:

```bash
mutation-journal organ list --path memory/organs.jsonl
```

Review decay state without writing:

```bash
mutation-journal organ review \
  --path memory/organs.jsonl \
  --evidence-bundle memory/evidence/p4-review.json
```

Suggest missing registry entries from already-recorded changed organs without writing:

```bash
mutation-journal organ suggest \
  --registry memory/organs.jsonl \
  --promotions memory/mutation-promotions.jsonl \
  --generations memory/generations.jsonl \
  --evidence-bundle memory/evidence/p4-suggest.json
```

`organ review` groups organ names by decay status and returns a structured `OrganReviewOutput` with per-organ findings, reasons, and evidence fields. The output has `schema_version: 0.1.0` and is described by `memory/organ-review.schema.json`. Missing registries are treated as empty. Review does not update the registry and does not remove files.

`organ suggest` reads promoted P2 promotion records and P3 generation lineage records, collects their `changed_organs`, and reports names that do not yet appear in the organ registry. Rejected or errored P2 promotion records are ignored because they did not pass the promotion gate. The output has `schema_version: 0.1.0` and is described by `memory/organ-registry-suggestion.schema.json`. The output is advisory and read-only: it does not append records, mark decay status, or choose organ metadata such as type and purpose. It also reports per-source load status and limits so a missing ledger is not silently confused with complete evidence.

Both read-only commands can write an optional EvidenceBundle sidecar with `--evidence-bundle`. The sidecar records the command output as the raw observation and carries P4 advisory limits as backed claims. The sidecar path must not overwrite the registry, promotion journal, or generation journal inputs.

The initial read-only rules are deterministic and local:

- duplicate `name` or duplicate `source` -> `duplicate`
- `failure_count > 0` -> `failed`
- explicit `decay_status` -> that status
- `usage_count == 0` -> `stale`
- no `related_goals` -> `candidate`
- otherwise -> `unknown`

## Boundary

P4 decay review is advisory. Marking an organ as `candidate`, `stale`, `duplicate`, or `failed` does not delete it. Any removal, replacement, Nix edit, Rust edit, or systemd unit change must be proposed as a separate mutation and pass through the P1/P2/P3 pipeline.

Registry suggestions have the same boundary. A suggestion means only that a recorded `changed_organs` value is not registered by exact organ name. Humans or later tooling must still create the `organ add` record explicitly.
