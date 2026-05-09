# ADR 0001: NixOS構成をゲノム、generationを系譜として扱う

- Status: Accepted
- Date: 2026-05-08

## Context

このプロジェクトは、AIがLinux上でコマンドを実行するアシスタントではなく、OS自身の構成を変異させる研究として始める。

NixOSでは、システム構成、パッケージ、module、service、user環境を宣言的に記述できる。変更後にはgenerationが残り、前の構成へ戻れる。この性質は復旧機能としてだけでなく、変更の系譜を読むためにも使える。

## Decision

Autopoietic OSでは、NixOS構成をOSの「ゲノム」として扱う。

対象は次を含む。

- `flake.nix`
- `flake.lock`
- NixOS modules
- Home Manager modules
- overlays
- package derivations
- host configuration

また、NixOS generationは単なる復旧点ではなく、accepted mutationによって生まれた系譜ノードとして扱う。

## Consequences

- 主要な変更はNix構成の差分として表現する。
- mutation IDとgeneration番号を紐づける必要がある。
- rollbackは「安全装置」だけではなく「系譜操作」として記録する。
- Nixで表現できない副作用は、このADRの範囲外として別に扱う必要がある。

## Alternatives considered

### Linuxコマンド履歴を主な状態表現にする

コマンド履歴は実行の痕跡としては使えるが、再現可能なOS状態そのものではない。変更理由や目的も構造化されない。

### dotfiles repositoryを中心にする

ユーザー環境の管理には向くが、systemd service、package derivation、host configurationまで含むOS全体のゲノムとしては弱い。

### コンテナイメージを世代として扱う

実験環境としては有効だが、NixOSのmodule合成やgenerationの意味を直接使えない。

## Follow-up

- generation metadata recorderを作る。
- mutation journalにparent generationとchanged organsを記録する。
- `self-state.json` に現在generationとflake inputsを含める。
