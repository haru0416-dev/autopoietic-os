# External Grounding Protocol

外部仕様に依存する判断では、モデルの記憶も、単一の公式ドキュメントも、そのまま確定根拠にしない。

## Iron law

version-sensitiveなclaimは、次の両方を満たすまで `confirmed` にしない。

1. executable probeがある。
2. lateral corroboratorがある。

どちらかが欠ける場合、そのclaimは `inconclusive`、`version-mismatched`、`stale`、または `conflicted` として扱う。

## Scope

このprotocolを使う対象は、変更されうる外部仕様である。

- Nix CLIのsubcommand、flag、default behavior
- NixOS module option名、型、default
- nixpkgsのattribute path、package名、version
- Home Manager option名、型、module behavior
- Rust crate API、feature flag、MSRV、edition互換性
- systemd unit optionや検証方法
- NixOS VM test / ISO image buildのAPIやmodule
- security advisory、deprecation、migration guide
- lockfileで解決されたtransitive dependency version

安定した言語基本仕様、完全にlocal codeだけで証明できる意味、ユーザーが明示した独自仕様にはこのprotocolを無理に適用しない。

## Required record

外部仕様に依存する実装・ADR・mutationでは、次を記録する。

```text
External grounding
- External surface:
- Stale risk:
- Local anchor:

Claims
- EG-001:
  Claim:
  Source:
  Source quality:
  Version fit:
  Executable probe:
  Lateral check:
  Decision:

Implementation constraints
- Use:
- Do not use:
- Required migration or compatibility note:
- Verification needed during execution-loop:

Handoff
- Confirmed facts:
- Conflicted or inconclusive facts:
- URLs / local paths:
```

## Decisions

- `confirmed` — source quality、version fit、executable probe、lateral corroboratorが揃っている。
- `version-mismatched` — claimは別versionでは正しいかもしれないが、local anchorには適用できない。
- `stale` — sourceが古い、日付やversionが不明、または関連releaseより前である。
- `conflicted` — credible sources、local types、runtime、docsが矛盾している。
- `inconclusive` — single-source、probeなし、横断確認なし、または必要なfactを確認できない。

実装制約に使えるのは `confirmed` のみである。

## Local anchor first

remote documentationを見る前に、まずlocal anchorを確認する。

例:

- `flake.lock` のnixpkgs revision
- `Cargo.lock` のcrate version
- installed CLIの `--version` / `--help`
- local NixOS module source
- local Rust type definition
- vendored documentation
- checked-in schema

`latest` docsは、local anchorとversionが合わない限り、そのまま適用しない。

## Executable probes

probeは、実装対象と同じlocal contextで実行できる確認を優先する。

例:

- `nix eval` でoptionやattributeを確認する。
- `nix flake check` でflake outputを確認する。
- `nix build` でpackageやISO targetを確認する。
- `cargo check` でcrate APIを確認する。
- `systemd-analyze verify` でunitを確認する。
- CLIの `--help` や `--version` を確認する。

production endpoint、cloud resource、payment、live system mutationなど副作用のあるprobeは、明示的な承認なしに実行しない。

## Lateral corroboration

単一sourceだけでは `confirmed` にしない。

横断確認として使えるものの例:

- official manual + release notes
- local source + official docs
- local type definition + compile check
- changelog + maintainer issue
- NixOS module source + `nix eval`
- crate docs + `cargo check`

権威あるsourceでも、local probeやversion fitと矛盾する場合はconfirmedにしない。

## Non-probeable facts

deprecation date、security advisoryのaffected range、vendor側の方針など、local executable probeで確認できないfactもある。

その場合は、少なくとも二つの独立した高品質sourceが一致していることを要求する。確認できない場合は `inconclusive` のままにする。

## No-network fallback

network accessが使えない場合は、次の順で扱う。

1. cached / vendored contentを使う。ただしcache dateやstalenessを明示する。
2. external sourceが未確認であることを記録し、local evidenceだけで判断できる範囲に限定する。
3. canonical URLと、ユーザーが確認すべき箇所を明示する。

それでも確定できないclaimは `inconclusive` とする。

## Handoff

実装に進む場合、`execution-loop` へ次を渡す。

- confirmed facts
- use / do not use constraints
- required migration note
- rerunすべきprobe
- inconclusiveなclaimと、それを解決するnext probe

bug cause、review claim、CI failureなどの判断に進む場合は、confirmed external factsだけを `evidence-gated-review` に渡す。
