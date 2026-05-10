# ADR 0017: P4はorgan registryとdecay reviewに限定する

- Status: Accepted
- Date: 2026-05-11

## Context

P0からP3までで、Autopoietic OSはobserve-only ISO、offline mutation verification、VM-tested promotion、install planning / seed verificationまでを段階化した。

次に必要なのは、追加されたCLI、module、service、package、devShellなどを、単なるファイル差分ではなく「organ」として追跡することである。ADR 0008は、依存や自作部品をorganとして記録・検証・削除できるかを判断基準に含めている。ADR 0010も、organ registryを変更する作業では意味理解を先に行うと決めている。

ただし、この段階でorganの自動昇格や自動削除まで入れると、mutation acceptance、usage観測、rollback、effect ledger、human policyの境界が混ざる。P4では、まずorgan registryを読み書きでき、decay候補を構造化してレビューできる最小範囲に留める。

## Decision

P4の範囲を、organ registryとdecay reviewに限定する。

P4で作るものは次の通り。

- organ registry record
  - name
  - organ type
  - source
  - purpose
  - created_by
  - usage_count
  - failure_count
  - related_goals
  - decay_status
- organ registry journal or store
  - 既存の`memory/organ.schema.json`と`OrganRecord`を基準にする
  - 追記可能なJSONLまたは明示的なregistry fileとして扱う
  - どの形式を採る場合もschemaで検証できるようにする
- organ review CLI
  - organを登録する
  - organ一覧を読む
  - decay candidate / stale / duplicate / failed を記録する
  - review結果を構造化して出力する
- decay review boundary
  - 使用回数、失敗回数、関連goal、重複、古さを根拠として候補化する
  - 自動削除はしない
  - Nix patch、Rust patch、systemd unit削除などの実変更は、別のmutation proposalとしてP1/P2/P3 pipelineを通す

P4では、organの自動昇格、自動削除、live system変更、依存追加の自動判断、remote/cloud registry、GUI、agentによる継続監視は扱わない。

## Consequences

- 「ある部品がなぜ存在するか」をmutationやgenerationとは別の軸で追いやすくなる。
- P1/P2/P3で出てくる`changed_organs`を、後からregistryに接続できる。
- 不要・重複・失敗し続ける部品を候補として見える化できる。
- 一方で、P4のdecay reviewは削除そのものを意味しない。削除や置換は通常のmutationとして扱う必要がある。
- usage_countやfailure_countの自動計測は、最初は弱くてもよい。P4では、人間またはtoolが明示的に記録できる形を優先する。

## Alternatives considered

### P4でorganの自動昇格まで行う

自律性は上がるが、何をorganとみなすかの判断がまだ弱い。P4では、まずregistryとreview記録を作る。

### P4でunused organを自動削除する

削除はNix構成、Rust workspace、systemd unit、memory schemaなど複数の境界に影響する。P1/P2/P3 pipelineを通さずに削除すると、rollbackと証跡が弱くなるため採用しない。

### organ registryをmutation journalに混ぜる

実装は簡単だが、mutationの成否とorganの寿命・用途・腐敗状態が混ざる。organは再利用性と意味を追う単位なので、registryとして分ける。

## Follow-up

- `mutation-journal`または新しいCLIにorgan registry操作を追加するかを決める。初期実装では`mutation-journal organ`に置く。
- `memory/organ.schema.json`を`OrganRecord`と同期して見直す。初期実装では`dev-shell`表記とoptional fieldをRust側に合わせる。
- organ registryの保存形式をJSONLにするか、snapshot JSONにするかを決める。
- `changed_organs`からregistry候補を作るread-only reviewを実装する。
- decay review結果をEvidenceBundleへ写像するかを検討する。
