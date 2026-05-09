# ADR 0013: P1はoffline mutation verifierに限定する

- Status: Accepted
- Date: 2026-05-09

## Context

P0では、Autopoietic OS ISOがobserve-only systemとしてbootできることを確認した。次の段階では、mutationを扱う入口を作る必要がある。

ただし、ここでいきなりlive systemを変更すると、失敗時の観測、journal、rollback境界がまだ弱い。AIによる自動patch生成も、検証器官がない状態では品質を判断しにくい。

ADR 0006は、mutationをverifier feedback付きの段階的pipelineで通すと決めている。P1では、その最初の実行可能な切片として、patch候補を隔離された環境で検証し、結果を構造化して残すところまでを扱う。

## Decision

P1の範囲を、offline mutation verifierに限定する。

P1で作るものは次の通り。

- mutation proposal format
  - goal
  - phase
  - changed paths
  - expected checks
  - patch body or proposal-relative patch path
  - side-effect declaration
- proposalを検証するCLI
  - live systemには適用しない
  - temporary worktreeまたはcopy上でpatchを適用する
  - Nix eval、`nix flake check`、必要なtargeted checkを実行する
  - 結果を構造化して返す
- mutation result journal
  - 成功、失敗、実行エラーを消さずに残す
  - 失敗理由と実行したcheckを記録する
- 最小限のtests
  - valid proposalが通る
  - malformed patchが落ちる
  - undeclared side effectを要求するproposalが落ちる
  - failed checkがjournalに残る

P1では、AIによる自動patch生成、live `nixos-rebuild switch`、automatic revert、generation lineage promotion、install workflow、GUI、heavy agent runtimeは扱わない。

## Consequences

- P1はmutationを「実行」する段階ではなく、「候補を検証して記録する」段階になる。
- live systemへの副作用を避けながら、mutation pipelineの入力形式と検証結果形式を早く固定できる。
- verifierが弱い領域は、P1の時点ではhuman reviewまたはmanual intent noteに残る。
- production ISO内部状態のblack-box確認は、P1本体ではなく別のP0強化または後続phaseで扱う。

## Alternatives considered

### P1でAI patch generatorまで作る

見た目の自律性は上がるが、検証器官が弱いままpatch生成を始めることになる。ADR 0005のobserve-first方針とも相性が悪い。

### P1でlive switchまで行う

研究の核心には近いが、effect ledger、revert、generation lineage、失敗時の隔離がまだ不足している。P1では扱わない。

### production status oracleをP1に含める

有用ではあるが、P1の主題はmutation verifier pipelineである。production ISOのread-only status oracleは、P0の検証強化として別に設計する方が境界が明確になる。

## Follow-up

- `mutation-runner verify --proposal <file>` の最小CLIを設計する。
- proposal schemaとresult schemaを追加する。
- temporary worktree上でpatchを適用する検証手順を実装する。
- side-effect declarationをeffect ledgerの語彙とそろえる。
- P2では、VM-tested mutation promotionとgeneration lineageへの接続を検討する。
