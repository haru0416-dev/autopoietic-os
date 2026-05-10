# Organ registry and decay review

P4 starts with a local, explicit organ registry. The goal is to record why reusable parts exist and to review decay without changing the live system.

## Registry records

Organ records use `OrganRecord` and `memory/organ.schema.json`. By default, `mutation-journal organ add` appends JSONL entries to `memory/organs.jsonl`.

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
mutation-journal organ review --path memory/organs.jsonl
```

`organ review` groups organ names by decay status and returns a structured `OrganReviewOutput` with per-organ findings, reasons, and evidence fields. Missing registries are treated as empty. Review does not update the registry and does not remove files.

The initial read-only rules are deterministic and local:

- duplicate `name` or duplicate `source` -> `duplicate`
- `failure_count > 0` -> `failed`
- explicit `decay_status` -> that status
- `usage_count == 0` -> `stale`
- no `related_goals` -> `candidate`
- otherwise -> `unknown`

## Boundary

P4 decay review is advisory. Marking an organ as `candidate`, `stale`, `duplicate`, or `failed` does not delete it. Any removal, replacement, Nix edit, Rust edit, or systemd unit change must be proposed as a separate mutation and pass through the P1/P2/P3 pipeline.
