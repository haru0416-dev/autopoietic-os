# ADR 0006: mutationはverifier feedback付きの段階的pipelineで通す

- Status: Accepted
- Date: 2026-05-08

## Context

Nix patchは、構文が正しいだけでは足りない。`nix flake check` が通っても、目的を満たすとは限らない。buildが成功しても、activation後のserviceが期待通りに動くとは限らない。

特にこのプロジェクトでは、build成功とintent alignmentの差が大きな研究課題になる。

## Decision

mutationは段階的なverifier pipelineを通す。

基本の順序は次の通り。

1. patch生成
2. patch適用
3. Nix syntax / eval check
4. `nix flake check`
5. build
6. VM build / VM boot
7. smoke test
8. intent check
9. accepted / failed / reverted の記録
10. 許可された場合のみlive switch

各段階の結果はmutation journalに戻す。失敗したmutationも削除せず、次の仮説と一緒に残す。

## Consequences

- LLMの単発生成ではなく、検証feedbackを使う設計になる。
- 失敗はノイズではなく進化履歴として扱える。
- mutation-runnerは単なるbuild scriptではなく、研究上の観測装置になる。
- verifierが弱い領域では、buildが通るだけの無意味な器官が増える危険が残る。

## Alternatives considered

### build成功だけでacceptedにする

速いが、意図に合わないmutationをacceptedにしやすい。

### 人間レビューだけでacceptedにする

初期には有効だが、自律度を上げる研究には不十分。

### VM検証を後回しにする

開発は楽になるが、OS変異の研究ではactivation後の挙動が重要になる。

## Follow-up

- mutation-runnerにphaseごとのresult型を作る。
- VM testの最小ケースを定義する。
- intent checkを最初は手動メモ、後で機械的checkへ拡張する。
