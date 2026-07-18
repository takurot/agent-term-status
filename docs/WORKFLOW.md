# 開発ワークフロー

`docs/TASK_PLAN.md` を参照して、指定されたタスク（例: I-02）の実装を進めてください。
全タスクは GitHub Issue として登録されており、スコープ・DoD・依存関係は Issue 本文に記載されています。

## 実装フロー

### 1. Issue 内容の確認

```bash
gh issue list --label phase-0 --label phase-1   # タスク一覧
gh issue view <番号>                             # 要件・DoD・依存関係の確認
```

- Issue の要件・背景・DoD（受け入れ条件）を把握する
- `docs/TASK_PLAN.md` の依存列を確認し、依存タスクがマージ済みであることを確認する
- 関連する `docs/SPEC.md` のセクションと既存コード・テストを調査する

### 2. ブランチ作成

- `feature/issue-<番号>-<簡潔な説明>` の形式でブランチを作成
  - 例: `feature/issue-2-ats-core-data-model`
- `main` ブランチから分岐すること

### 3. 適切な Skills の選択と TDD 実装

- タスクに応じて以下の Skills を選択して使うこと:
  - `tdd-workflow` — 新機能・バグ修正・リファクタ全般（テストファースト）
  - `rust-testing` — Rust テスト TDD（cargo-llvm-cov でカバレッジ確認）
  - `rust-patterns` — 所有権・エラー処理・trait 設計の確認
  - `verification-loop` — 実装完了前の検証
- **Red → Green → Refactor** のサイクルを守る
- SPEC §20 のテスト区分に従い、該当するものをすべて作成する:
  - Unit Test（イベント変換・状態遷移・TTL・サニタイズ・テーマ解決 など）
  - Integration Test（Hook JSON → 表示、fail-open、Socket 権限、複数セッション分離 など）
  - E2E / Fault Injection（I-23 のハーネス、I-24 のスイート導入後）

### 4. テストとコード品質の確認

```bash
cargo test --workspace                                        # 全テスト実行
cargo test --test '*'                                         # 統合テスト
cargo clippy --all-targets --all-features -- -D warnings      # lint
cargo fmt --all -- --check                                    # フォーマット確認
```

- 既存のテストが壊れていないことを確認
- 新機能のテストを追加

### 5. パフォーマンスの確認（該当タスクのみ）

Hook 経路・Daemon に触れる変更は SPEC §16 の目標値を確認すること:

| 項目 | 目標 |
|------|------|
| Hook CLI 実行時間（`ats ingest`） | ≤ 50 ms |
| イベント遅延（hook → render） | ≤ 150 ms |
| Daemon 常駐メモリ | ≤ 30 MB |
| イベントスループット | ≥ 100 events/sec |

- **Note**: 正確な計測のため、`cargo build --release` を完了させてから計測すること
- 計測ハーネスは I-23（Test & perf harness）で導入されるものを使う

### 6. コミット & プッシュ & PR 作成

- コミットメッセージ形式: `<type>(<scope>): <description>`
  - type: `feat`, `fix`, `test`, `docs`, `refactor`, `chore`
  - scope はクレート名を基本とする（例: `feat(ats-state-engine): add TTL expiry handling`）
- 適切な粒度でコミットを分割

```bash
git push -u origin <ブランチ名>
gh pr create --title "feat(<scope>): <説明> (I-XX)" \
  --body "Closes #<番号>

## Summary
...

## Test plan
- [ ] ..."
```

### 7. サブエージェントによるコードレビュー & PR へのコメント投稿

- `code-review` サブエージェント等でコードレビューを実施
- セキュリティ DoD を持つタスク（I-07, I-13, I-15 など）はセキュリティ観点（入力検証・Socket 権限・ログ redaction・fail-open）も必ずレビューする
- 指摘内容を PR に投稿:
  ```bash
  gh pr comment <PR番号> --body "<レビュー内容>"
  ```

### 8. 指摘事項への対処

- CRITICAL / HIGH の指摘は必ず修正する
- MEDIUM の指摘は可能な限り対処する
- 修正後に再コミット & プッシュ

### 9. CI がオールグリーンになるまで対処

```bash
gh pr checks <PR番号>          # CI 状態確認（全グリーンまでポーリング）
gh run view                    # 失敗時の詳細確認
```

- 失敗した場合は原因を特定して修正し、再プッシュ
- 全チェックがパスするまで繰り返す

### 10. 実装内容の最終検証 & マージ

- Issue の DoD を一つずつ確認し、検証内容をまとめる
- MVP 受け入れ条件（SPEC §21）に関連するタスクは `docs/TASK_PLAN.md` のトレーサビリティ表で対応する条件を確認する
- 問題がなければマージ:
  ```bash
  gh pr merge <PR番号> --squash --delete-branch
  ```
- 問題があれば修正し、実装 → 検証 → レビューを正しい実装になるまで繰り返す

## チェックリスト

- [ ] Issue の要件・DoD を把握した
- [ ] `docs/TASK_PLAN.md` で依存タスクのマージ済みを確認した
- [ ] `main` からブランチを作成した
- [ ] 適切な Skills を選択して使った
- [ ] テストを先に書いた（TDD: Red → Green → Refactor）
- [ ] ユニット・統合（該当すれば E2E / Fault Injection）テストが全てパスする
- [ ] `cargo clippy -- -D warnings` / `cargo fmt --check` がエラーなし
- [ ] コミット & プッシュ & PR を作成した
- [ ] サブエージェントでコードレビューを実施し PR にコメントを投稿した
- [ ] 指摘事項に対処した
- [ ] CI が全てグリーン
- [ ] Issue の DoD を検証し問題なし
- [ ] PR をマージした

## 注意事項

- 既存のテストを壊さないこと
- 依存関係のあるタスク（`docs/TASK_PLAN.md` 参照）がマージされていることを確認
- **fail-open を壊さない**: `ats` 側の障害で AI エージェントの動作を止めない。Hook 経路は内部エラーでも成功を返す
- **プライバシー不変条件を守る**: プロンプト本文・ファイル内容・コマンド文字列・API キー・フルパスをデフォルトで収集・保存しない
- **Hook 経路（`ingest` / `event`）は 50 ms 以内** を意識した実装にする
- macOS（tmux / iTerm2）を優先。Windows / Linux はクロスコンパイル維持のみで品質保証対象外
- RISK は MVP では「ATTENTION の強調表示」として扱う（SPEC §13 参照）

## サブエージェントへの委任時の注意（完了報告の自己申告を鵜呑みにしない）

複数の Issue を並行して worktree + バックグラウンドサブエージェントに委任する際、サブエージェントが `status: completed` を返しても、それは自己申告に過ぎない。実際には `cargo test` の実行途中で応答を打ち切っており、変更はコミットされていなかった、というケースが起こりうる。

**必須の検証手順**（サブエージェントの完了報告を受け取ったら）:

1. 該当 worktree で `git log --oneline -3` と `git status --short` を自分の Bash ツールで直接実行し、コミットが実在するか確認する。
2. 未コミットの変更が残っている場合は、`cargo test` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo fmt --all -- --check` を自分で実行して結果を確認する（サブエージェントの報告文だけで判断しない）。
3. 作業が中断されていた場合は、同じサブエージェント（同一 agentId）に `SendMessage` で再開を依頼するか、フルコンテキストを与えて再開させる。新しい `Agent` を素で立ち上げると worktree パスなどの前提知識を失い、二重作業や迷子になるリスクがある。
4. 全チェックがグリーンであることを自分の目で確認してから、初めて push / PR 作成に進む。

**理由**: バックグラウンドエージェントの「完了」通知は、ターン数上限や応答の打ち切りでも発火することがあり、実際のタスク完了を保証しない。特に `cargo test` のようなビルドに数分かかる処理を跨ぐ場合、サブエージェントが結果を待たずに応答してしまう可能性がある。

### 打ち切られたサブエージェントの再開のさせ方

`status: completed` の通知が来ても、結果本文が「バックグラウンドの `cargo test` 完了を待っています」のように未完了を示している場合がある。この場合、単に「続けて」と再開させるだけでは同じ理由で再び打ち切られる可能性がある。`SendMessage` で再開を指示する際は、以下を明示的に指定すること:

1. バックグラウンド実行 (`run_in_background: true` 相当のシェル `&`) ではなく、**フォアグラウンドで** `cargo test` / `cargo clippy` 等を実行し、実際の出力を読み切るまで応答を返さないこと。
2. 「計画」ではなく「実行結果」を報告すること（テスト結果のサマリ、コミットSHA、CI状態など、推測ではなく実際に見た値）。
3. 完了を主張する前に、そのエージェント自身が `git log --oneline -3` / `git status --short` を実行してコミットの実在を確認すること。

こう明示すると、バックグラウンド実行に起因する早期打ち切りが解消されるケースが多い。

## コードレビューで見るべき追加観点（並列化リファクタ）

「処理を並列化するだけ」など性能改善のみを謳うPRでも、直列ループを並列化する過程で **出力そのものが変わる**ことがある（例: `BTreeMap` でキー重複排除した結果、解決済み集合が暗黙に変化する）。本プロジェクトでは Daemon のイベント処理・Renderer への配信がこの類のリファクタ対象になりやすい。code-review サブエージェントやセルフレビューで直列→並列化のPRを見る際は、次の2点を必ず確認する:

1. **副作用のタイミング**: 旧コードが「最終値が確定する前」に何か（キューへの追加、ログ、共有状態の更新）をしていた場合、それを「最終値確定後にまとめて実行」に変えると、たとえ最終的に選ばれる値が同じでも副作用の集合が変わりうる。特に `Map<Key, _>` に集約する形へ変えると、同一キーへの複数回の書き込みが黙って上書きされ、以前は生き残っていた副作用が消えることがある。イベントの重複排除・順序逆転処理（State Engine）ではこの点に特に注意する。
2. **エラーのfail-fast性**: `try_join_all`（最初のエラーで即座に返る）を `stream::iter(...).buffer_unordered(N).collect()` に置き換えると、バッチ全体が完了するまでエラーが返らなくなり、複数エラー発生時にどれが返るかも非決定的になる。fail-fast挙動が必要なら `TryStreamExt::try_collect()` や手動の early-return ループを使い、そのトレードオフをコメントで明記する。

再現テストは「並列で動いていることの証明（タイミング）」だけでなく、「並列化前後で出力が一致することの証明（正確性）」を別テストとして追加すること。
