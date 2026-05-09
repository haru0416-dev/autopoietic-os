# Anti-goals

Autopoietic NixOS intentionally does not start as:

- a natural-language shell;
- a chat UI for Linux commands;
- a generic dotfiles generator;
- a convenience wrapper around `nixos-rebuild`;
- a GUI desktop shell;
- a voice-operated assistant;
- an unbounded autonomous root agent;
- a minimal-change NixOS assistant that avoids structural evolution.

The project also avoids treating build success as full correctness. A mutation must be evaluated for intent alignment, side effects, later reuse, and decay.
