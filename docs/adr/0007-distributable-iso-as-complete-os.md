# ADR 0007: 最終成果物はISOとして配布できる完全OSを目指す

- Status: Accepted
- Date: 2026-05-08

## Context

Autopoietic NixOSは、単なるNixOS設定リポジトリやdotfiles集として終わらせない。

研究の中心は、AIがNix構成を変異させ、generationとして自己を更新し、記憶と器官を持つOSを作ることにある。そのため、最終的な成果物は「既存NixOSに追加で入れるツール」では弱い。

ユーザーがISOから起動・導入でき、最初から自己観測、mutation journal、effect ledger、agent runtime、organ registryを備えた環境として配れる必要がある。

## Decision

Autopoietic NixOSの長期的な最終成果物は、ISO形式で配布できる完全OSとする。

短期的にはflake-based NixOS seedとして開発するが、設計上は次を前提にする。

- installer ISOまたはlive ISOを生成できる。
- ISOにはAutopoietic core toolsを含める。
- 初回起動時点でobserve-only modeが動く。
- host identity、memory directory、mutation workspaceを初期化できる。
- 導入後のsystem generationとmutation lineageを紐づけられる。
- 将来的には、ISO自体もmutation lineageから生成されたartifactとして記録する。

## Consequences

- flake outputsに `nixosConfigurations` だけでなく、ISO image build targetを持たせる必要がある。
- 初期設定は既存マシン前提ではなく、fresh install / live boot を考慮する。
- `/etc/nixos` 相当の構成、memory store、mutation workspaceの初期化手順が必要になる。
- Home Manager設定は、特定ユーザー名に固定しすぎない設計にする必要がある。
- hardware configurationは、開発host用とISO用を分ける。
- agent runtimeは、初期状態では危険なlive mutationを行わず、observe-onlyで起動する。

## Non-goals

初期段階では次を作らない。

- 独自Linux distributionをゼロから作ること。
- NixOS installer全体の再実装。
- GUI installer。
- secure bootやdisk encryptionの完全設計。
- autopoiesis modeをISO初期状態で有効にすること。

## Alternatives considered

### 既存NixOSに追加するflake templateとして配る

導入は簡単だが、OSとしての自己同一性が弱い。既存hostの状態に強く依存し、研究対象が「完全OS」ではなく「設定レイヤー」になりやすい。

### Home Manager moduleとして配る

ユーザー環境の器官化には有効だが、generation、systemd、NixOS module、installer artifactを含むOS全体の研究には足りない。

### container imageとして配る

実験槽としては便利だが、boot、system generation、host identity、hardware adaptationを扱えない。

## Follow-up

- ISO用の `hosts/iso/` または `images/iso/` 構成を追加する。
- flake outputにISO build targetを追加する。
- 初期ユーザー、host identity、memory初期化の方針をADR化する。
- live ISOでのobserve-only introspectionの最小成功条件を定義する。
- ISO artifactにもmutation metadataを付与する設計を検討する。
