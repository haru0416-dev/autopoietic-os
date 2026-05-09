# Ontology

## Genome

The declarative Nix representation of the system: `flake.nix`, `flake.lock`, NixOS modules, Home Manager modules, overlays, packages, and host configuration.

## Body

The activated runtime system: services, timers, CLIs, shells, users, logs, generated files, and project environments.

## Mind

The agent layer that reads state, proposes mutations, interprets verifier feedback, and decides whether a tool should become an organ.

## Mutation

A proposed change to the genome, represented as a patch plus metadata. A mutation may be pending, accepted, failed, or reverted.

## Generation

A NixOS generation produced by an accepted mutation. Generations are lineage nodes linked to mutation IDs.

## Organ

A recurring capability promoted into durable Nix structure: NixOS module, Home Manager module, package derivation, systemd unit, devShell, overlay, or CLI.

## Memory

The searchable operational record of goals, mutations, effects, verifier outcomes, activation results, reuse, decay, and compensation.

## Effect

Any side effect outside pure Nix configuration, such as writing user files, touching external APIs, modifying databases, or generating indexes.

## Decay

Loss of fit between organs and current goals, including unused organs, duplicate modules, rising build failures, goal drift, or excessive complexity.
