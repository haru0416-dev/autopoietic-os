# ADR 0015: P2はVM-tested mutation promotionに限定する

- Status: Accepted
- Date: 2026-05-09

## Context

P1では、mutation proposalをlive systemに適用せず、隔離されたworktreeで検証してjournalへ残せるようにした。ただし、P1の`verified`は、patch適用、Nix eval、`nix flake check`、限定されたtargeted checkを通ったという意味にとどまる。

OSの変異では、buildやevalが通っても、boot後のservice、ファイル配置、observe-only境界が期待通りとは限らない。ADR 0006は、mutationを段階的なverifier pipelineで通し、VM build / VM boot / smoke testを独立した段階として扱うと決めている。

P3はinstall workflowとgeneration lineage接続を扱うが、その前に「どのmutationをVMで試し、installやgeneration lineageへ渡してよい候補にするか」を定義する必要がある。

## Decision

P2の範囲を、VM-tested mutation promotionに限定する。

P2で作るものは次の通り。

- P1 verification resultを入力にしたpromotion入口
  - `verified`なmutationだけを対象にする
  - proposal、patch、検証結果の参照をpromotion recordに残す
  - P1が検証したproposal contentとP2でpromoteするproposal contentをSHA-256 fingerprintで結びつける
  - P1が検証したbase genomeとP2でpromoteするbase genomeをSHA-256 root fingerprintで結びつける
  - P1の検証を省略したmutationはP2へ進めない
- VM検証手順
  - candidate worktreeまたはcopy上でmutationを再現する
  - 対象NixOS configurationをbuildする
  - VM boot checkを実行する
  - phaseごとのsmoke assertionを実行する
  - 実行したcheck名、結果、log参照、失敗理由を記録する
- promotion result journal
  - `promoted`、`rejected`、`error`を区別する
  - 成功だけでなく、boot failureやsmoke assertion failureも消さずに残す
  - P3が参照できるpromotion evidence IDを持たせる
- P3へ渡すgeneration lineage evidence
  - mutation ID
  - P1 verification evidence reference
  - P2 promotion evidence reference
  - parent genome revisionまたはdigest
  - candidate target configuration
  - proposal fingerprint
  - verified root fingerprint / promotion root fingerprint
  - changed paths / changed organs
  - 実行したVM checks

P2では、`verified`、`promoted`、`accepted`を次のように分ける。

- `verified`: P1で、隔離worktree上のpatch適用と静的・build系checkを通った状態。
- `promoted`: P2で、VM bootとsmoke assertionを通り、P3や後続phaseへ渡せる候補になった状態。
- `accepted`: mutationを実際のgeneration lineageへ取り込む明示的な判断。P2の`promoted`だけでは`accepted`とはみなさない。

P2では、AI patch generation、live `nixos-rebuild switch`、install workflow、installed systemへの書き込み、automatic revert、organ registry promotion、GUI、remote/cloud promotionは扱わない。

## Consequences

- P1の`verified`が過大な意味を持たなくなる。
- installやlive mutationの前に、boot後の挙動を観測できる。
- P3は、P2 promotion evidenceを前提にgeneration lineageを作れる。
- VM testが弱い領域では、`promoted`であってもintent alignmentを完全には保証しない。必要なmanual noteや追加assertionはpromotion recordに残す。
- VM実行は時間がかかるため、P1より重いgateになる。すべてのproposalを無条件にP2へ進めるのではなく、P1 resultとphase policyで対象を絞る必要がある。

## Alternatives considered

### P1の`verified`をそのままP3へ渡す

実装は早いが、boot後の失敗をinstallやgeneration lineageの段階で初めて見つけることになる。ADR 0006の段階的pipelineとも合わない。

### P2で`accepted`まで決める

VM検証が通ったことと、mutationを系譜へ取り込む判断は同じではない。acceptanceにはgeneration ledger、effect ledger、人間またはpolicyによる明示的な判断が関わるため、P2では扱わない。

### P2でlive switchとautomatic revertを実装する

研究上は重要だが、失敗時の副作用、generation rollback、effect ledger補償を同時に扱うことになる。P2ではVM上のpromotionに閉じる。

## Follow-up

- `mutation-runner promote` の最小CLIを設計する。
- promotion result schemaとRust型を追加する。
- P1 resultからP2 promotionへ進める入力検証を実装する。
- P2用の最小VM boot / smoke checksを定義する。
- P3実装では、P2 promotion evidenceをgeneration lineage recordの入力として使う。
