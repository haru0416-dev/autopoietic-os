# Mutation Protocol

Every mutation should follow this sequence.

1. Observe current state with `os-introspect`.
2. Select relevant memory: prior goals, mutations, effects, failures, and organ history.
3. Run semantic review when the change depends on understanding existing meaning or invariants.
4. Ground version-sensitive external facts with local anchors, executable probes, and lateral corroboration, following `docs/protocols/external-grounding.md`.
5. Form a mutation hypothesis.
6. Generate a Nix patch, not an imperative command sequence.
7. Record the draft mutation in the journal.
8. Apply the patch in a controlled worktree.
9. Run static checks, beginning with `nix flake check` where available.
10. Build the target system or package.
11. Run VM or smoke tests for activation behavior.
12. Record all failures with phase and next hypothesis.
13. If accepted and authorized, switch the live system.
14. Link the new generation to the mutation ID.
15. Record non-Nix side effects in the effect ledger.
16. Revisit the organ registry for promotion, decay, merge, or removal.

The protocol intentionally separates pure mutation from side effects. Nix rollback can restore configuration lineage; it cannot undo arbitrary writes, network calls, or user data changes.
