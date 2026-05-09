# ADR 0014: P3はinstall workflowとgeneration lineage接続に限定する

- Status: Accepted
- Date: 2026-05-09

## Context

P0では、observe-only ISOがbootできることを確認した。P1では、mutation proposalをlive systemに適用せず、隔離worktreeで検証して結果をjournalに残せるようにした。

次の大きな問題は、検証済みmutationをどのように実際のOS generationへ接続するかである。ただし、その前にP2でVM-tested promotionを定義する必要がある。P3はP2を前提に、VMで昇格可能と判断されたmutationを、install workflowとgeneration lineageへつなぐ段階として切る。

P3で扱うinstall workflowは、完全なinstaller UIではない。目的は、Autopoietic OSを対象ディスクへ導入し、導入後のsystem generationとmutation IDを追跡できる最小経路を作ることである。

## Decision

P3の範囲を、install workflowとgeneration lineage接続に限定する。

P3で作るものは次の通り。

- 最小install workflow
  - 対象rootを明示的に指定する
  - dry-run / plan表示を持つ
  - 実ディスクやlive systemへの破壊的操作は、明示承認なしに実行しない
  - installer UI、partitioning wizard、GUIは含めない
- generation lineage record
  - mutation ID
  - parent generation
  - resulting generation
  - activation or install result
  - changed organs
  - verifier evidence reference
- installed systemへの初期Autopoietic memory seed
  - identity
  - mutation result journal
  - generation ledger
  - effect ledger
- post-install verification
  - installed rootでNixOS configurationが評価できること
  - generation lineage entryが読めること
  - mutation IDとgeneration recordが対応していること

P3では、AI patch generation、live autonomous mutation、automatic revert、organ registry promotion、full installer UX、remote/cloud installは扱わない。

## Consequences

- P3で初めて、mutation verificationとNix generation lineageが接続される。
- install workflowは最小でよいが、対象rootや破壊的操作の境界を曖昧にしない必要がある。
- P3はP2のVM-tested promotionを前提にする。P2なしにP3を実装しない。
- generation recordは、単なるbuild logではなく、後から「どのmutationがどのgenerationを生んだか」を読むための系譜になる。
- 実ディスク操作やinstall commandは副作用を伴うため、effect ledgerとの接続が必要になる。

## Alternatives considered

### P3でfull installerを作る

配布OSとしては魅力的だが、partitioning、network、user setup、hardware variationまで扱うと範囲が広がりすぎる。P3では最小install workflowに絞る。

### P3でlive mutationまで進める

generation lineageとは関係が深いが、自律変異とlive switchを同時に入れると失敗時の境界が曖昧になる。P3ではinstallとlineageに限定する。

### generation lineageを後回しにする

installだけ先に作ると、導入後のsystemがどのmutationから生まれたか追えない。ADR 0001の「generationを系譜として扱う」方針に反する。

## Follow-up

- P2 ADRで、verified mutationをVM-tested promotionへ昇格する条件を定義する。
- P3実装前に、install workflowで許容する副作用とeffect ledger記録形式を決める。
- generation ledger schemaとRust型をP3用途に見直す。
- install workflowのdry-run出力をsnapshot testできる形にする。
