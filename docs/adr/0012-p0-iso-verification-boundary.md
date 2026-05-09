# ADR 0012: P0 ISO検証境界を分ける

- Status: Accepted
- Date: 2026-05-09

## Context

P0では、Autopoietic NixOS ISOが単にbuildできるだけでなく、observe-only systemとしてboot検証済みである必要がある。

ただし、検証には二つの性質がある。

1. ISO内部のAutopoietic設定、memory directory、Rust CLI、negative behaviorを詳しく検証すること。
2. 配布対象であるproduction ISO artifactそのものがbootして `multi-user.target` に到達すること。

NixOS test instrumentationをISOへ入れると、test driverはguest内で `machine.succeed(...)` や `wait_for_unit(...)` を使える。これは詳細assertionには適している。一方で、それはproduction ISOそのものではない。production ISOにtest backdoorを入れずに同じ詳細assertionを行うには、別途read-only status oracleやserial protocolを設計する必要がある。

## Decision

P0のISO検証境界を次のように分ける。

- 詳細なobserve-only assertionは、production ISOと同じhost configuration/core moduleを使ったtest-instrumented ISOで行う。
- production ISO artifactそのものは、test instrumentationなしのblack-box VM boot checkで検証する。
- production ISO checkは、serial consoleから `multi-user.target` 到達を観測する。
- ISO host configurationには、通常画面を残す `console=tty0` と、headless検証用の `console=ttyS0,115200n8` を両方入れる。

これにより、P0では「配布artifactがbootする」ことと「Autopoietic observe-only構成がboot後に正しい」ことを、別々の検証で満たす。

## Consequences

- P0はproduction ISOの直接boot evidenceを持つ。
- 詳細assertionは引き続きtest-instrumented ISOに依存する。
- production ISOにはNixOS test backdoorや`jq`などのtest harness dependencyを入れない。
- serial console parameterはproduction ISOにも入るため、headless/debug bootに有用だが、boot logがserialへ出ることはdistribution behaviorの一部になる。
- P0のproduction artifact boot evidenceはBIOS CD-ROMとUEFI CD-ROMの両方を含む。
- 将来、production ISO内のAutopoietic stateをblack-boxで検証したい場合は、test instrumentationではなくread-only status oracleを設計する。

## Alternatives considered

### test-instrumented ISOだけをP0 evidenceにする

詳細assertionは十分にできるが、配布artifactそのものがbootする証拠が弱い。ISO配布を長期目標にするADR 0007と相性が悪い。

### production ISOだけをblack-boxで見る

artifact boot evidenceとしては強いが、`/etc/autopoietic/*.json`、memory directory、CLI behavior、negative metadata validationをguest内で確認できない。P0のobserve-only baselineとして情報が不足する。

### production ISOへtest backdoorを入れる

検証は楽になるが、production artifactとtest harnessの境界が崩れる。最小distribution方針にも反する。

## Follow-up

- production ISO向けのread-only Autopoietic status oracleを検討する。
- P0 phase commit時に、test-instrumented detailed checksとproduction black-box checkの両方のevidenceをcommit messageまたはrelease noteに残す。
