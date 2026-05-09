# ADR 0004: 中核CLIはRust workspaceで実装する

- Status: Accepted
- Date: 2026-05-08

## Context

初期CLIは小さく作れるが、最終的にはNix構成、journal、JSON schema、systemd、process実行、VM runner、patch適用、effect ledgerを扱うことになる。

これらはOS寄りの処理であり、長く保守する中核になる。プロトタイプしやすさより、型で構造を固定できること、単体バイナリとして配布しやすいこと、エラー境界を明確にできることを優先する。

## Decision

Autopoietic NixOSの中核CLIはRust workspaceで実装する。

想定する初期workspaceは次の通り。

```text
crates/
  autopoietic-core/
  os-introspect/
  mutation-journal/
```

`autopoietic-core` には共通型を置く。

- `SelfState`
- `MutationRecord`
- `EffectRecord`
- `GenerationRecord`
- `OrganRecord`

CLI binaryは `clap`、JSON入出力は `serde`、application errorは `anyhow`、library errorは必要に応じて `thiserror` を使う。

## Consequences

- schemaと実装型のズレを早く検出できる。
- Nix flakeはCargo workspaceをbuildする形に寄せる。
- Pythonやshellのプロトタイプは長期実装として残さない。
- 初期開発速度は少し落ちるが、mutation-runner以降の堅さを優先できる。

## Alternatives considered

### Pythonで実装する

観測CLIの試作は速いが、長期的な型安全性、配布、OS統合の面で弱い。

### Shell scriptで実装する

Nixやsystemdとの接続は簡単だが、ledgerやschemaの扱いが壊れやすい。

### Goで実装する

単体バイナリには向くが、このプロジェクトではRustの型、エラー処理、Nix ecosystemとの相性を優先する。

## Follow-up

- 既存のPythonプロトタイプをRust workspaceに置き換える。
- `flake.nix` を `rustPlatform.buildRustPackage` 前提に変更する。
- 共通型を `autopoietic-core` に集約する。
