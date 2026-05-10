# ADR 0016: Evidence bundleとcanonical comparisonで情報受け渡しを固定する

- Status: Accepted
- Date: 2026-05-10

## Context

Autopoietic OSでは、mutation proposal、検証結果、VM promotion、install plan、generation lineage、effect ledgerが段階的につながる。

P1とP2では、proposal fingerprintやroot fingerprintで入力のすり替わりを防ぎ始めた。P3では、install seed manifestとread-only seed verificationにより、計画されたseed fileと実際のtarget root上のfileを比較できるようになった。

ただし、このまま各段階が個別のJSONやログを直接読み合うと、次の問題が残る。

- 生の観測、正規化された比較対象、AIやtoolの解釈が混ざる。
- timestamp、temporary path、実行順序、UUIDのような揺れる値が比較結果を汚す。
- AIが作った要約や判断が、根拠から切り離されて次段階の入力になりやすい。
- `verified`、`promoted`、`planned` のような状態が、何を根拠にしたclaimなのか後から追いにくくなる。
- 比較不能、証跡不足、古い証跡、複数候補のような状態を単なる失敗か成功に潰しやすい。

Autopoietic OSでは、AIの判断が次のAIの判断材料になる。ここで情報が汚れると、誤った説明や過大なclaimがmutation pipelineに残り続ける。

## Decision

phase間で渡す情報は、raw observation、canonical fact、derived claimを分けて扱う。

新しい境界概念として、Evidence BundleとCanonical Comparisonを導入する。

### Evidence Bundle

Evidence Bundleは、phase間で渡す証跡の単位である。

少なくとも次を持つ。

- bundle ID
- schema version
- phase
- subject
  - mutation ID
  - proposal fingerprint
  - root fingerprintまたはgeneration ID
- input references
  - pathまたはlogical source
  - SHA-256 digest
  - schema version
- observations
  - 実際に観測したcheck、file、command result、VM result、ledger entry
  - raw artifactへの参照
  - raw artifact digest
- canonical facts
  - 比較しやすい形に正規化した事実
  - volatile fieldを除いた値
- comparison results
  - 何と何を比較したか
  - 比較結果
  - 比較不能だった理由
- claims
  - `verified`、`promoted`、`planned`、`installed` などの判断
  - backingとして使ったobservationやcomparison result
  - limits
- data quality
  - その情報をどの強さで使ってよいか

AIやtoolの要約は、Evidence Bundleのclaimとして残してよい。ただし、claimはraw artifactまたはcanonical factへのbackingを持つ。backingのないclaimは、次段階の判断材料にしない。

### Raw / Canonical / Derived

情報は次の三層に分ける。

- Raw
  - 実際に観測したデータ。
  - CLI output、VM log、JSONL entry、flake.lock、schema file、target root上のfile contentなど。
- Canonical
  - 比較用に正規化したデータ。
  - field order、timestamp、temporary pathなど、比較に不要な揺れを取り除く。
- Derived
  - tool、policy、AI、人間が出した判断。
  - `this mutation is promoted`、`seed files matched`、`installed root is not yet evaluated` など。

DerivedはRawまたはCanonicalへの参照なしに渡さない。

### Comparison status

比較結果はboolにしない。少なくとも次を使う。

- `matched`
- `mismatched`
- `missing`
- `incomparable`
- `stale`
- `ambiguous`
- `error`

`incomparable` は、schema version不一致や比較対象の型違いなど、比較そのものが成立しない場合に使う。

`stale` は、入力digest、root fingerprint、generation IDなどが現在の対象と一致しない場合に使う。

`ambiguous` は、同じselectorに複数の候補がある場合に使う。曖昧な候補を自動で選ばない。

### Data quality

Evidence Bundle内のfactやclaimには、情報の強さを持たせる。

初期語彙は次の通り。

- `raw`
- `observed`
- `canonicalized`
- `verified`
- `derived`
- `stale`
- `ambiguous`
- `unknown`

`verified` は、対応するcomparison resultまたはcheck resultがあり、それがbundle内で参照可能な場合にだけ使う。

`derived` は、AI、人間、policy、toolが解釈した情報であることを示す。`derived`なclaimを使う場合は、backingとlimitsを読む必要がある。

### Volatile field handling

schemaやrecordごとに、比較で使うfieldを分ける。

- identity fields
  - mutation ID、proposal fingerprint、promotion ID、generation IDなど。
- evidence fields
  - check status、exit code、content digest、root fingerprintなど。
- volatile fields
  - timestamp、temporary path、runtime log path、generated UUID、実行順に依存する値など。
- derived fields
  - summary、reason、AI note、manual noteなど。

Canonical comparisonでは、identity fieldsとevidence fieldsを中心に比較する。volatile fieldsはraw artifactとして残すが、通常の一致判定からは外す。

### AI handoff

AIへ渡す情報は、自由文の要約だけにしない。

AI-facing handoffは次の形を基本にする。

```text
Summary:
Canonical facts:
Evidence refs:
Claims:
Limits:
Open uncertainty:
```

AIはこのhandoffを読んでよいが、次段階の入力として信頼してよいのは、Evidence refsとCanonical factsに結びついたclaimだけである。

## Consequences

- phase間の受け渡しが少し重くなる。
- schema、Rust型、CLI出力にEvidence Bundle用の型が必要になる。
- AIが書いた説明を、そのまま事実として扱いにくくなる。
- 一方で、後から「何を見て、何と比較し、なぜそのclaimになったか」を追いやすくなる。
- P1/P2/P3の境界で、`verified`、`promoted`、`planned`、`installed` の意味が証跡に結びつく。
- 不一致だけでなく、missing、stale、ambiguous、incomparableを明示できる。

## Alternatives considered

### 各phaseのJSONL journalを直接読み合う

実装は早いが、raw observation、比較結果、解釈が混ざりやすい。後続phaseが「前のphaseの結論」だけを読み、根拠を見落とす危険がある。

### AI要約をhandoffの主データにする

読みやすいが、根拠と切れやすい。要約の誤りが次の判断へ伝播するため、このプロジェクトでは採用しない。AI要約はclaimとして残し、backingを必須にする。

### すべてのraw logをそのまま次phaseへ渡す

情報は失われにくいが、比較には向かない。timestampやtemporary pathのような揺れがノイズになり、AIやtoolが重要な差分を見落としやすくなる。

### boolの一致判定だけを使う

単純だが、missing、stale、ambiguous、incomparableを表現できない。fail closedにしたい場面でも、原因の種類が消えて次の修正に使いにくい。

## Follow-up

- `autopoietic-core` に `EvidenceBundle`、`CanonicalFact`、`ComparisonReport`、`ComparisonStatus`、`DataQuality`、`ProvenanceRef`、`DigestRef` の最小型を追加する。
- `memory/` にEvidence Bundle schemaを追加する。
- P1 verification resultとP2 promotion recordをEvidence Bundleへ写像する設計を作る。
- `install-plan` と `install-verify` の出力を、将来のEvidence Bundleへ接続できるようにする。
- volatile field policyをschemaごとに文書化する。
- AI-facing handoff formatをimplementation noteに落とす。
