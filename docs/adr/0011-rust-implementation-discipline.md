# ADR 0011: Rust実装の書き方を制限する

- Status: Accepted
- Date: 2026-05-08

## Context

Autopoietic NixOSの中核CLIはRust workspaceで実装する。これらのCLIは、自己観測、mutation journal、effect ledger、将来のmutation-runnerやos-mutatorを担う。

ここはOSの内部器官にあたる。動けばよいという実装ではなく、長く読めて、検証できて、Nix構成やmemory schemaとずれにくい書き方が必要になる。

Rustの書き方をagentごとに任せると、error handling、module分割、clone、async、testing、clippy方針が揺れる。揺れた実装はsemantic reviewやmutation生成の入力として扱いにくい。

## Decision

Rust実装は、このADRの制約に従う。

### Error handling

- application binaryでは `anyhow::Result` を使う。
- library crateでは、呼び出し側が分岐すべきerrorに `thiserror` を使う。
- `?` だけで文脈が落ちる箇所には `.context(...)` または `.with_context(...)` を付ける。
- production codeで `unwrap()` を使わない。
- `expect()` はプログラミングエラーに限り、理由が読めるmessageを付ける。
- error messageは小文字始まり、末尾句点なしを基本にする。

### Ownership and borrowing

- 不要な `.clone()` を避け、まずborrowで設計する。
- 引数は `&String` ではなく `&str`、`&Vec<T>` ではなく `&[T]` を優先する。
- 大きな値はcloneせずmoveできないか検討する。
- 小さく自明なenumは `Copy` をderiveしてよい。
- 共有が必要になった場合だけ `Arc` / `Rc` / `Mutex` / `RwLock` を使う。

### API design

- `main.rs` は薄く保ち、意味のある処理は `lib.rs` またはfeature moduleに移す。
- moduleはtype別ではなくfeature別に分ける。
- workspace dependency inheritanceを使う。
- publicなstruct / enumは、必要に応じて `Debug`, `Clone`, `PartialEq` をderiveする。
- 外部に公開するenum / structは、将来互換性が必要なら `#[non_exhaustive]` を検討する。
- validationは境界で行う。内部ではparse済み・検証済みの型を渡す。
- 柔軟な入力が必要なAPIでは `impl Into<T>` を使ってよいが、過剰に使わない。

### Serialization

- JSON / JSONL schemaに対応する型は `autopoietic-core` に集約する。
- schemaとRust型を意図的にずらす場合は、理由をADRまたはimplementation noteに残す。
- JSON field名がRust識別子と衝突する場合は `#[serde(rename = "...")]` を使う。
- library crateの `Serialize` / `Deserialize` は、将来的にfeature gateを検討する。ただし初期MVPではschema型が中核なので直接deriveしてよい。

### Async

- 初期MVPでは不要なasync runtimeを入れない。
- asyncが必要になったら、導入前にADRを書く。
- tokio等を入れる場合は、lockを`.await`越しに保持しない。
- file IOをasync化するなら、runtimeとbackpressureの設計を先に決める。

### Memory and performance

- 最初は明瞭さを優先する。
- サイズが予測できるcollectionでは `with_capacity` を検討する。
- tight loopや大きなJSON処理で不要なallocationが見えたら、根拠を持って最適化する。
- performance目的の複雑化は、測定か明確なhot pathなしに行わない。

### Testing

- behaviorを固定するunit testまたはintegration testを追加する。
- CLIは、少なくともsmoke testかsnapshotに相当する出力確認を持つ。
- async導入時は `#[tokio::test]` を使う。
- property-based testingは、parser、schema変換、ledger appendのような境界に優先して使う。

### Clippy and formatting

各crateは最低限次を入れる。

```rust
#![deny(clippy::correctness)]
#![warn(clippy::suspicious, clippy::style, clippy::complexity, clippy::perf)]
```

実装後は次を通す。

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Release profile

ISO配布や実機導入用のrelease profileは、必要になった段階で明示的に設定する。

候補は次の通り。

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true

[profile.dev.package."*"]
opt-level = 3
```

ただし、初期段階でrelease profileを固定しすぎるとdebugやbuild時間に影響するため、ISO build ADRまたはrelease ADRで確定する。

## Consequences

- Rust codeの自由度は下がる。
- 一方で、agent間で実装スタイルが揺れにくくなる。
- schema、ledger、CLIの境界が読みやすくなる。
- `anyhow` と `thiserror` の使い分けにより、application errorとlibrary errorを分けられる。
- `main.rs` を薄くする方針により、今後 `os-introspect` と `mutation-journal` のロジックはlibraryへ移す必要がある。

## Alternatives considered

### Rust標準の慣習に任せる

一般的なRust projectなら十分な場合もある。しかし、このリポジトリではagentが継続的に読み書きするため、揺れの少ない制約が必要である。

### 全crateで最初から厳密なlibrary設計にする

理想的ではあるが、MVP初期には重い。まず制約を置き、徐々に `main.rs` からlibraryへ移す。

### performance方針を最初から強く固定する

自己観測CLIの初期段階では、performanceより正しさと観測可能性が重要である。hot pathが見えるまで過剰最適化しない。

## Follow-up

- `mutation-journal` と `os-introspect` の処理を `main.rs` からlibrary moduleへ分離する。
- `autopoietic-core` に必要なlibrary errorを追加する段階で `thiserror` を導入する。
- CLI smoke testをintegration testとして追加する。
- schemaとRust型の対応をtestで確認する。
- release profileをISO build設計時に確定する。
