# ADR 0003: Nix generationで戻らない副作用をeffect ledgerで扱う

- Status: Accepted
- Date: 2026-05-08

## Context

NixOS generationで戻せるのは、主に宣言的なシステム構成である。

AIが自律度を上げると、Nix構成の変更だけでは済まなくなる。たとえば、ユーザーファイルの生成、PDF indexの作成、外部APIの呼び出し、cache更新、project directoryへの書き込みが起きる。

これらはNix rollbackだけでは戻らない。ここを曖昧にすると、「戻せるから大胆に変えられる」という前提が崩れる。

## Decision

Nix構成の変異と、Nix外の副作用を別の台帳で記録する。

- Nix patchとgenerationはmutation journalに記録する。
- ファイル書き込み、外部呼び出し、DB更新、index生成などはeffect ledgerに記録する。

effect entryには、少なくとも次を持たせる。

- effect ID
- mutation ID
- 種類
- 対象
- 可逆性
- 補償操作
- 確認方法
- risk

## Consequences

- Nix rollbackの限界を明示できる。
- live mutationやautopoiesisに進む前に、副作用の範囲をレビューできる。
- 完全な巻き戻しではなく、補償操作として扱う必要がある。
- effect ledgerを読まないmutation generatorは、過去の副作用を見落とす危険がある。

## Alternatives considered

### すべてをNix store内に閉じ込める

理想的だが、ユーザー作業、project files、外部API、index更新を扱う研究としては狭すぎる。

### 副作用をjournal logsだけで追う

ログは観測には使えるが、補償や可逆性の判断に必要な構造を持たない。

### 副作用を禁止する

observe-onlyやmutate-draftでは有効だが、live mutationやautopoiesisの研究には進めない。

## Follow-up

- effect schemaを定義する。
- `mutation-journal effect` でeffect entryを追加できるようにする。
- mutation-runnerが副作用を伴う段階を明示的に区切る。
