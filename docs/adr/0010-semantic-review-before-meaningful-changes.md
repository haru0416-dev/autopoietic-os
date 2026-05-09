# ADR 0010: 意味理解が必要な変更ではsemantic reviewを先に行う

- Status: Accepted
- Date: 2026-05-08

## Context

Autopoietic OSでは、AIがNix構成、Rust製CLI、systemd unit、memory schema、mutation runnerを読み、次の変更案を作る。

このとき、コードや設定の「なぜ」を曖昧なまま推測すると、その誤解が次のmutation、ADR、memory、organ registryへ伝播する。特にこのプロジェクトでは、過去の分析が未来の変更判断の材料になるため、浅い要約やそれらしい意図の捏造は通常のコードレビューより危険である。

実装の読み替えは理解ではない。名前や構文を言い換えただけの説明は、semantic-preserving rewriteに耐えない。たとえばローカル変数名を変えたり、同じ意味の分岐構造へ書き換えたりしただけで分析内容が変わるなら、それは意味理解ではなく字面の追跡である。

## Decision

意味理解が必要な変更では、実装前にsemantic reviewを行う。

semantic reviewでは、少なくとも次を守る。

- module hypothesisを先に置く。
- meaningful unitごとにmechanical behaviorとdomain intentを分ける。
- `Why` は必ず `confident`、`plausible`、`UNKNOWN — probe: ...` のいずれかで書く。
- fabricated certaintyを禁止する。
- invariants、failure modes、backward slice、forward sliceを明示する。
- semantic-preserving rewriteに耐えるか確認する。
- generated / vendored codeは無理に解釈せず、skip理由を明示する。

`UNKNOWN` は失敗ではない。次に何を読めば確かめられるかを示すほうが、もっともらしい誤説明より価値がある。

## When required

次の場合はsemantic reviewを先に行う。

- Rust workspaceの中核型やCLIを変更する。
- NixOS moduleやHome Manager moduleの意味を変える。
- mutation protocol、memory schema、effect ledger、organ registryを変更する。
- `mutation-runner`、`os-mutator`、`promotion-engine` のような判断ロジックを変更する。
- 既存moduleを蒸留、自作、削除、統合する。
- 過去のmutationやADRを根拠に変更する。

軽微なtypo、format、明らかな一行修正、空ディレクトリ追加のような変更では必須ではない。

## Required output shape

semantic reviewの記録は、必要に応じてADR、implementation note、またはmemory entryへ残す。

基本形は次の通り。

```text
Module hypothesis:

Unit:
  What mechanical:
  What domain intent:
  Why:
  Invariants:
  Failure modes:
  Connections ←:
  Connections →:

Hypothesis verdict:
Open questions:
Risk hotspots:
Implementation implications:
```

すべての `Why` は根拠の強さを持つ。

- `confident` — 直接の証拠がある。
- `plausible` — 推論として妥当だが未確認。
- `UNKNOWN — probe: ...` — 次に何を読めば確かめられるかを示す。

## Consequences

- 実装前の読み取りに時間を使う。
- ただし、誤った設計理由がmemoryに残るリスクを下げられる。
- 後続のAI agentが「それらしい説明」を真実として引き継ぐことを防ぎやすい。
- 大きな変更はsemantic reviewからexecution loopへ明示的にhandoffする流れになる。
- 変更の根拠が、コードの字面ではなくinvariantとsliceに結びつく。

## Alternatives considered

### 実装しながら理解する

小さな修正では十分な場合もある。しかし、このプロジェクトでは理解結果がmemoryやmutation判断に残るため、誤解したまま編集すると後続の判断まで汚染する。

### コメントや名前だけで意図を判断する

名前は手がかりにはなるが、invariant、caller discipline、副作用、履歴的理由までは保証しない。

### Whyを推測で埋める

採用しない。推測は `plausible` と明示する。根拠が弱いなら `UNKNOWN — probe` と書く。

## Follow-up

- Rust workspace化の前に、既存のPython prototypeをsemantic review対象にするか、破棄対象として扱うかを決める。
- `autopoietic-core` の型を作る前に、schemaとmutation protocolのsemantic reviewを行う。
- `memory` にsemantic review結果を残す形式を検討する。
- `os-mutator` が過去のsemantic reviewを参照できるようにするか検討する。
