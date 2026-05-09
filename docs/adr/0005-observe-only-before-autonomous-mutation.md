# ADR 0005: 自律変異の前にobserve-onlyの自己観測を作る

- Status: Accepted
- Date: 2026-05-08

## Context

このプロジェクトの最終的な関心は、OSが自分を変異させることにある。ただし、自己観測が弱いままmutationを始めると、AIは現在の構成、過去の失敗、副作用、器官の状態を読めない。

その状態でpatchを生成しても、進化ではなく場当たり的な自動改造になりやすい。

## Decision

最初のMVPはobserve-onlyにする。

最初に作るものは `os-introspect` と `self-state.json` であり、live mutationやautopoiesisは後回しにする。

`self-state.json` には少なくとも次を含める。

- identity
- flake inputs
- Nix files
- Home Manager / NixOS moduleの概況
- systemd units
- installed packages
- generations
- journal summary
- project summary
- memory ledger paths
- pain point candidates

## Consequences

- 自律性より観測可能性を先に得る。
- mutation generatorの入力形式を早く固定できる。
- 実装初期に派手なデモは出にくい。
- 後続の器官化、腐敗検出、patch生成の品質が上がる。

## Alternatives considered

### 先にAI patch generatorを作る

見た目の進捗は速いが、入力状態が薄いとpatchの品質を評価できない。

### 先にlive mutationを作る

研究の核心には近いが、失敗時の記録と観測が不足する。

### 先にGUIやチャットUIを作る

このプロジェクトの中核ではない。UIは後から載せられる。

## Follow-up

- `os-introspect` のRust実装を作る。
- `self-state.schema.json` をRust型と合わせる。
- observe-only modeをNixOS module optionとして保持する。
