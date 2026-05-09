# ADR 0002: mutationはコマンド列ではなくNix patchとして表現する

- Status: Accepted
- Date: 2026-05-08

## Context

AIにOSを操作させる設計では、自然言語からshell commandを生成し、その場で実行する形になりやすい。しかし、その方式では変更が一時的になり、レビューしにくく、世代として残らない。

このプロジェクトで観察したいのは、AIがどのようにOSを変えるかである。したがって、AIの出力は実行ログではなく、読める差分でなければならない。

## Decision

AIが生成する主な変更単位をNix patchにする。

`os-mutator` はコマンドを直接実行しない。入力としてgoal、`self-state.json`、memory、organ registryを受け取り、出力としてpatchとmutation metadataを返す。

patchの適用、build、VM検証、live switchは `mutation-runner` が担当する。

## Consequences

- AIの思考結果をGit diffとしてレビューできる。
- buildやVM testのfeedbackをpatch修正ループに戻せる。
- 変更はNixの構造に残るため、後から器官化・腐敗・削除を扱いやすい。
- 一方で、Nix patch生成の難しさがプロジェクトの中核リスクになる。

## Alternatives considered

### AIにshell commandを直接実行させる

短期的には動きやすいが、自己進化OSではなく操作代行になる。副作用も追いにくい。

### AIに自然言語の提案だけを出させる

安全ではあるが、OSが自分を変異させる研究にならない。

### AIに完全な新規configurationを生成させる

既存の構成、履歴、器官を保った変異になりにくい。既存個体を育てるという目的に合わない。

## Follow-up

- `os-mutator` の最初の出力形式を決める。
- patchに期待するverification planを添付する。
- build成功とintent alignmentを別の評価軸として記録する。
