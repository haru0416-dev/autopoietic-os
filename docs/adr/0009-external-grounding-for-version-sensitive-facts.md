# ADR 0009: version-sensitiveな外部事実はprobeと横断確認なしにconfirmed扱いしない

- Status: Accepted
- Date: 2026-05-08

## Context

Autopoietic OSは、NixOS、nixpkgs、Home Manager、Rust、systemd、VM test、ISO image generationなど、多くの外部仕様に依存する。

これらは時間とともに変わる。CLI option、NixOS module option、Rust crate API、flake outputの形、systemd unitの挙動、nixpkgsのpackage名、ISO生成方法は、記憶だけで扱うには危険である。

このプロジェクトでは、AI自身がNix構成を変異させる。外部仕様を誤って理解したままmutationを作ると、build失敗だけでなく、間違った器官化、不要な自作、過剰な依存追加につながる。

## Decision

version-sensitiveな外部事実は、次の両方を満たすまで `confirmed` として扱わない。

1. executable probeがある。
2. lateral corroboratorがある。

どちらかが欠ける場合、その事実は `inconclusive`、`version-mismatched`、`stale`、または `conflicted` として扱う。

ここでいうversion-sensitiveな外部事実には、少なくとも次を含める。

- Nix CLIのsubcommand、flag、default behavior
- NixOS module option名と型
- nixpkgs package名、attribute path、version
- Home Manager option名と型
- Rust crate API、feature flag、MSRV、edition互換性
- systemd unit optionや挙動
- VM test frameworkのAPI
- ISO image buildに必要なNixOS moduleやflake output
- security advisory、deprecation、migration guide

詳細な運用手順は [`docs/protocols/external-grounding.md`](../protocols/external-grounding.md) に置く。ADRは方針を固定し、protocolは実際の記録形式、decision label、no-network fallback、handoffを定義する。

## Required workflow

外部仕様に依存する実装を行う前に、次を記録する。

```text
External surface:
Stale risk:
Local anchor:
Claim:
Source:
Version fit:
Executable probe:
Lateral check:
Decision:
Implementation constraints:
```

`confirmed` になったclaimだけを実装制約として使う。

Decision labelは次を使う。

- `confirmed`
- `version-mismatched`
- `stale`
- `conflicted`
- `inconclusive`

## Local anchor

まずlocal anchorを確認する。

例:

- `flake.lock` のnixpkgs revision
- `Cargo.lock` のcrate version
- installed CLIの `--version`
- local NixOS module source
- vendored documentation
- generated type definition
- checked-in schema

remote docsが最新でも、local anchorとversionが合わなければ、そのclaimはそのまま適用しない。

## Executable probe

実行可能な確認を優先する。

例:

- `nix eval` でoptionやattributeを確認する。
- `nix flake check` でflake outputを確認する。
- `nix build` でpackageやISO targetを確認する。
- `cargo check` でRust APIを確認する。
- `systemd-analyze verify` でunitを確認する。
- CLIの `--help` や `--version` を見る。

probeが副作用を持つ場合は、ユーザー承認なしにproduction endpointやlive systemへ触れない。

## Lateral corroborator

単一sourceだけではconfirmedにしない。

横断確認として使えるものの例:

- official manualとrelease notes
- local sourceとofficial docs
- local type definitionとcompile check
- changelogとmaintainer issue
- NixOS module sourceと`nix eval`
- crate docsと`cargo check`

sourceの権威が高くても、version fitやlocal probeと矛盾する場合はconfirmedにしない。

## Consequences

- 外部仕様に依存する変更は少し遅くなる。
- しかし、NixやRustのAPI記憶違いによるmutation失敗を減らせる。
- nixpkgsやHome Managerのlatest docsを、local flakeにそのまま適用しない癖がつく。
- ISO生成、VM test、systemd integrationのような変わりやすい領域で、実装前に根拠を残せる。
- `os-mutator` や `mutation-runner` も、将来的には外部factをmemoryに記録し、再利用できるようにする必要がある。

## Alternatives considered

### 公式ドキュメントをそのまま信じる

公式ドキュメントは重要だが、local versionと一致するとは限らない。latest docsを古いlockfileへ適用すると壊れる。

### local buildが通ればconfirmedとみなす

build成功は必要だが十分ではない。sourceが主張している内容、version fit、意図との一致は別に確認する必要がある。

### モデルの記憶を使う

このプロジェクトでは採用しない。記憶は仮説の入口にはできるが、confirmed factにはできない。

## Follow-up

- Rust workspace化の前に、使用するcrate APIをlocal anchorとprobeで確認する。
- ISO生成用のNixOS moduleを追加する前に、現在のnixpkgs revisionでbuild targetをprobeする。
- `mutation-runner` にexternal fact確認結果を添付できるmetadataを用意する。
- `memory` にexternal claims / probes / decisionsのschemaを追加するか検討する。
