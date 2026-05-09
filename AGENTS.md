# Autopoietic OS Agent Instructions

このリポジトリで作業するagentは、実装や設計判断の前にADRを読む。

## Session start

1. `CONTEXT.md` がrootに存在する場合は、最初に読む。
2. `docs/adr/README.md` を読む。
3. 作業に関係するADRを読む。
4. 実装が外部仕様に依存する場合は、`docs/protocols/external-grounding.md` を読む。
5. 既存コードやmoduleの意味理解が必要な場合は、ADR 0010に従ってsemantic reviewを先に行う。

## Required ADR awareness

最低限、次の決定を前提にする。

- NixOS構成はゲノム、generationは系譜として扱う。
- AIの主出力はコマンド列ではなくNix patchにする。
- Nix generationで戻らない副作用はeffect ledgerに記録する。
- 中核CLIはRust workspaceで実装する。
- 自律変異の前にobserve-onlyの自己観測を作る。
- mutationはverifier feedback付きpipelineで通す。
- 最終成果物はISO配布できる完全OSを目指す。
- ただし形は、NixOSを基盤にした最小ディストリビューションとする。
- version-sensitiveな外部事実はprobeと横断確認なしにconfirmed扱いしない。
- 意味理解が必要な変更ではsemantic reviewを先に行う。
- Rust実装はADR 0011の制約に従う。

## Implementation discipline

- 読まずに推測で実装しない。
- ADRと矛盾する変更が必要な場合は、先に新しいADRを書くか、既存ADRをsupersedeする。
- Nix / nixpkgs / Home Manager / Rust crate / systemd / ISO buildに関する現在仕様は、モデル記憶だけで判断しない。
- 便利さだけで依存を増やさない。既存部品を使う、蒸留する、自作する、の順に検討する。
- Rust codeでは、applicationは`anyhow`、library errorは`thiserror`、薄い`main.rs`、workspace dependency inheritance、clippy/fmt/check/testを基本にする。
- live mutationや副作用を伴う操作は、明示的な承認なしに行わない。
