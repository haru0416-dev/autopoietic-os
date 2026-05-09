# ADR

このディレクトリには Autopoietic NixOS の Architecture Decision Record を置く。

ADR は、あとから「なぜそうしたのか」を読むための記録であり、実装の説明書ではない。決定、捨てた選択肢、受け入れた不利益を短く残す。

## 一覧

| ADR | 状態 | 決定 |
| --- | --- | --- |
| [0001](0001-nixos-configuration-as-genome.md) | Accepted | NixOS構成をゲノム、generationを系譜として扱う |
| [0002](0002-mutations-are-patches-not-commands.md) | Accepted | AIの主出力をコマンド列ではなくNix patchにする |
| [0003](0003-effect-ledger-for-non-nix-side-effects.md) | Accepted | Nix generationで戻らない副作用をeffect ledgerで扱う |
| [0004](0004-rust-workspace-for-system-tools.md) | Accepted | 中核CLIはRust workspaceで実装する |
| [0005](0005-observe-only-before-autonomous-mutation.md) | Accepted | 自律変異の前にobserve-onlyの自己観測を作る |
| [0006](0006-verifier-guided-mutation-pipeline.md) | Accepted | mutationはverifier feedback付きの段階的pipelineで通す |
| [0007](0007-distributable-iso-as-complete-os.md) | Accepted | 最終成果物はISOとして配布できる完全OSを目指す |
| [0008](0008-minimal-nixos-based-distribution.md) | Accepted | NixOSを基盤にした最小ディストリビューションとして設計する |
| [0009](0009-external-grounding-for-version-sensitive-facts.md) | Accepted | version-sensitiveな外部事実はprobeと横断確認なしにconfirmed扱いしない |
| [0010](0010-semantic-review-before-meaningful-changes.md) | Accepted | 意味理解が必要な変更ではsemantic reviewを先に行う |
| [0011](0011-rust-implementation-discipline.md) | Accepted | Rust実装の書き方を制限する |
| [0012](0012-p0-iso-verification-boundary.md) | Accepted | P0 ISO検証境界をtest-instrumented詳細検証とproduction black-box bootに分ける |
| [0013](0013-p1-offline-mutation-verifier-boundary.md) | Accepted | P1はoffline mutation verifierに限定する |
| [0014](0014-p3-install-workflow-and-generation-lineage-boundary.md) | Accepted | P3はinstall workflowとgeneration lineage接続に限定する |
| [0015](0015-p2-vm-tested-mutation-promotion-boundary.md) | Accepted | P2はVM-tested mutation promotionに限定する |

## フォーマット

新しいADRは次の形を基本にする。

```markdown
# ADR NNNN: タイトル

- Status: Proposed | Accepted | Superseded
- Date: YYYY-MM-DD

## Context

## Decision

## Consequences

## Alternatives considered

## Follow-up
```
