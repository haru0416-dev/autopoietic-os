# Install planning and generation lineage

P3 begins with a dry-run install-plan boundary. The goal is to connect P2 promotion evidence to a generation lineage record before any target root or disk mutation is possible.

## Scope

`mutation-runner install-plan` reads promotion evidence from `memory/mutation-promotions.jsonl` by default and selects a record by `--promotion-id` or `--mutation-id`. If `--mutation-id` matches more than one promotion record, the command rejects the plan and requires `--promotion-id` to avoid binding generation lineage to the wrong evidence.

The selected record must be `promoted` and must include non-empty passing VM check evidence. Rejected, errored, incomplete, or non-passing promotion evidence cannot enter P3 generation lineage.

The command requires:

- an explicit absolute `--target-root`;
- `--parent-generation`;
- `--resulting-generation`.

By default, the command prints a `GenerationRecord` as JSON and does not write any journal. Passing `--record` appends the same record to `memory/generations.jsonl`. The generation journal path must not be inside the install target root.

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

## Safety boundary

This initial P3 slice does not run `nixos-install`, does not call `nixos-rebuild`, does not partition disks, and does not write to the target root. The only optional write is the local generation journal append under `--record`.

Real install execution, installed memory seeding, effect ledger writes for target-root side effects, and post-install NixOS evaluation are deferred until their external command behavior is grounded and the user explicitly approves those side effects. Later install execution must transition lineage from `planned` to `installed` or `failed`; planned records must not be treated as accepted installed generations.
