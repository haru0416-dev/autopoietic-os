# ADR 0008: NixOSを基盤にした最小ディストリビューションとして設計する

- Status: Accepted
- Date: 2026-05-08

## Context

Autopoietic NixOSは、NixOS上にツールを追加しただけの環境ではない。一方で、Linux distributionをゼロから作るわけでもない。

目指す形は、NixOSを基盤にしながら、自己観測・変異・検証・記憶・器官化に必要なものだけを精査して組み込んだ、最小構成のディストリビューションである。

ここでいう最小構成は、単にパッケージ数が少ないという意味ではない。OSの目的に対して説明できないものを入れない、という意味での最小性である。

## Decision

Autopoietic NixOSは、NixOSを基盤にした最小ディストリビューションとして設計する。

採用するものは、次の基準を満たす必要がある。

- 自己観測に必要である。
- Nix構成の変異に必要である。
- mutationの検証に必要である。
- memory、effect ledger、generation lineageの維持に必要である。
- organとして昇格するだけの反復性や意味がある。
- ISO配布時の初期体験に必要である。

既存のNixOS module、package、service、toolが要件を満たす場合は使う。要件に対して過剰な場合は、設定を削り、機能を限定し、薄いwrapperや小さなmoduleへ蒸留する。要件に合わない場合は、自作する。

## Distillation policy

既存のものを使うか、自作するかは次の順で判断する。

1. 既存のNixOS / nixpkgs / Home Manager機能で目的を満たせるか。
2. その機能は最小構成に対して過剰ではないか。
3. 過剰な場合、設定やmodule分割で蒸留できるか。
4. 蒸留しても目的に合わない場合、自作する価値があるか。
5. 自作したものは、organとして記録・検証・削除できるか。

自作は最後の手段ではあるが、避けるべきものではない。このOSの目的に対して既存部品が大きすぎる、曖昧すぎる、観測しにくい、または副作用を管理しにくい場合は、自作する。

## Consequences

- flakeやmodule構成は、汎用NixOS設定集ではなく、このディストリビューションの構成として扱う。
- 依存を追加するときは、なぜ必要かをADR、module comment、またはorgan metadataに残す。
- 便利さだけを理由にpackageを増やさない。
- 大きな外部ツールを入れる前に、蒸留できるかを検討する。
- Rust製の中核CLIは、この最小OSの内部器官として扱う。
- GUI、音声、重いagent runtime、クラウド同期などは初期ISOには入れない。

## Minimal initial distribution

初期ISOに含める候補は、現時点では次に限定する。

- NixOS base system
- flakes有効化済みNix
- Git
- Rust製Autopoietic core tools
  - `os-introspect`
  - `mutation-journal`
  - later: `mutation-runner`
- systemd units for observe-only runtime
- memory directory initialization
- mutation workspace
- minimal editor or recovery shell tools
- Home Manager integration, if it is needed for user-level organs

含めないものは次の通り。

- full desktop environment
- large AI runtime by default
- live mutation by default
- general-purpose developer package bundle
- broad media / research / writing tools before they are organs

## Alternatives considered

### NixOS設定テンプレートとして提供する

導入しやすいが、ディストリビューションとしての境界が曖昧になる。どこまでがOSの身体で、どこからがユーザーの既存環境なのかを観測しにくい。

### NixOSに大量の便利ツールを加えたbattery-included OSにする

短期的には使いやすいが、最小性、器官化、腐敗観測の研究に向かない。最初から肥大化した身体になる。

### 既存の軽量ディストリビューションを基盤にする

最小性だけなら可能だが、NixOS generation、module system、flake、derivationをゲノムとして使う設計から外れる。

## Follow-up

- 初期ISOに含めるpackageのallowlistを作る。
- 依存追加時の記録形式を決める。
- `organ-registry` に「既存採用 / 蒸留 / 自作」の由来を持たせる。
- `self-reviewer` に依存肥大化の検出を入れる。
- ISO構成を `hosts/iso/` として開発hostから分離する。
