# agent-term-status 仕様書

AIコーディングエージェント状態可視化CLIツール

- 文書バージョン: 1.0
- 対象バージョン: MVP 0.1
- 対象OS: macOS
- 対象ユーザー: CLI型AIコーディングエージェントを並列利用するソフトウェア開発者

---

## 1. 概要

### 1.1 目的

`agent-term-status` は、Claude Code などのAIコーディングエージェントが現在どのような状態にあるかを、ターミナルの色・ペイン枠・タブタイトル・バッジ・通知によって視覚化するローカルCLIツールである。

本ツールが解決する中心課題は「AI内部の処理状況を詳細に表示すること」ではなく、次の問いに即座に答えることである。

> ユーザーは、今このターミナルに注意を向ける必要があるか。

### 1.2 設計思想

AIツール固有のイベントを、少数の「ユーザー行動状態」へ正規化する。内部処理の詳細（thinking, planning, reading 等）は保持しつつ、表示上はユーザー行動に基づく状態へ集約する。

```
AIツール固有イベント
    ↓
イベント正規化 (Provider Adapter)
    ↓
セッション状態判定 (State Engine)
    ↓
ユーザー注意状態 (Agent State)
    ↓
ターミナル表現 (Renderer)
```

### 1.3 中核資産

本ツールの中核資産は以下の3層であり、これらの信頼性を最優先する。

1. **Provider層**: 異なるAIツールのイベントを共通形式へ変換
2. **State Engine**: 不完全で順序が乱れるイベントから安定した状態を生成
3. **Renderer層**: ターミナルごとの機能差を吸収

テーマや制御シーケンスはこれらの上に成り立つ付加機能である。

---

## 2. 用語定義

| 用語 | 定義 |
|------|------|
| Provider | AIツール（Claude Code, OpenCode 等）のイベントソースおよびその変換アダプタ |
| Session | 1つのAIエージェント実行単位。session.id で識別 |
| Agent State | ユーザー行動を表す表示状態（IDLE, WORKING, ATTENTION, RISK, RESULT, ERROR, UNKNOWN） |
| Activity | 状態に付随する短い活動ラベル（"Running tests" 等） |
| Renderer | ターミナル表現の出力先（tmux, iTerm2, macOS Notification） |
| Normalized Event | Provider間で共通の、本ツール内部イベント形式 |

---

## 3. スコープ

### 3.1 MVP (0.1) に含める機能

- Claude Code Hook との連携（Provider: `claude`）
- セッション単位の状態管理
- State Engine（状態遷移、優先順位、TTL、タイムアウト）
- Local Daemon（Unix Domain Socket 経由）
- Renderer
  - tmux（ペイン枠色・ペインタイトル）
  - iTerm2（バッジ・タブタイトル、プロファイル切り替えオプション）
  - macOS Notification
- YAML によるテーマ・設定
- 状態の自動復旧（TTL、Heartbeat補助、Crash Recovery）
- CLI（手動イベント送信、status、list、reset、doctor、install/uninstall、theme）
- ローカル完結動作
- ログ保存期間および無効化設定
- Homebrew Tap による配布

### 3.2 MVP に含めない機能

- AIの思考内容・プロンプト本文・コード内容の解析
- クラウドへのデータ送信
- AIエージェント自体の実行
- tmux・ターミナルエミュレータの代替
- 複数端末間の同期
- Webダッシュボード・チーム管理
- 危険コマンドの強制ブロック（RISKは表示のみ）
- AI利用料金・トークン量の集計
- Windows・Linux正式対応（クロスコンパイルは維持するが品質保証対象外）
- Codex CLI 正式サポート（Experimental のみ）
- OpenCode Provider（Phase 2）
- Generic Shell Integration（Phase 3）
- VS Code・Cursor 連携、メニューバーアプリ（Phase 3）

### 3.3 RISK状態の扱い

MVPでは、RISK状態は内部仕様として実装するが、初期UIでは **ATTENTIONの強調表示** として扱う。独立した「危険操作検知機能」としては宣伝しない（セクション13参照）。

---

## 4. システムアーキテクチャ

```
┌─────────────────────────────────────────────┐
│ Event Sources (MVP)                         │
│   Claude Code Hooks / Manual CLI            │
└──────────────────────┬──────────────────────┘
                       │ Native Event (JSON stdin)
                       ▼
┌─────────────────────────────────────────────┐
│ Provider Adapter                            │
│   claude-provider                           │
└──────────────────────┬──────────────────────┘
                       │ Normalized Event
                       ▼
┌─────────────────────────────────────────────┐
│ Local Event Broker (Daemon)                 │
│   Unix Domain Socket                        │
│   Session Registry / Validation / Dedup     │
└──────────────────────┬──────────────────────┘
                       ▼
┌─────────────────────────────────────────────┐
│ State Engine                                │
│   State Machine / Priority / Timeout        │
│   Parent/Subagent Aggregation               │
└──────────────────────┬──────────────────────┘
                       │ Agent State
                       ▼
┌─────────────────────────────────────────────┐
│ Rendering Engine                            │
│   Theme Resolution / Capability Detection   │
│   Rate Limiting                             │
└─────────┬──────────────┬─────────────┬──────┘
          ▼              ▼             ▼
       iTerm2          tmux       Notification
```

### 4.1 Standalone Mode（フォールバック）

Daemonが起動していない場合、CLIが直接Rendererへ出力する standalone mode を提供する。ただし standalone mode では複数イベント競合解決・TTL・通知抑制は限定される。Daemonは起動時に standalone で変更された表示を検知・引き継ぐことを推奨する。

---

## 5. コンポーネント仕様

### 5.1 CLI

#### 5.1.1 コマンド名

- フルネーム: `agent-term-status`
- 推奨短縮名: `ats`

配布物は `agent-term-status` バイナリを提供し、Homebrew formula にて `ats` のシンボリックリンク（または同等のエイリアス）を生成する。`presence` というコマンド名は一般語衝突の懸念から使用しない。

以下、本仕様書では `ats` を用いて記述する。

#### 5.1.2 サブコマンド一覧

```
ats install <provider> [--scope user|project|local] [--dry-run]
ats uninstall <provider> [--scope user|project|local]
ats ingest --provider <name>            # JSONをstdinから受信（Hook向け）
ats event <state> [--activity <label>]  # 手動状態送信
ats reset [--all | --session <id>]
ats status [--session <id>]
ats list [--json]
ats doctor
ats theme list
ats theme preview <name>
ats theme apply <name>
ats daemon start [--foreground]
ats daemon stop
ats daemon status
ats logs [--tail] [--level <level>]
```

- `event` の `<state>` は Agent State 名（小文字）を受け取る（例: `working`, `attention`, `result`）
- すべてのフック起点のコマンド（`ingest`, `event`）は **50ms以内** の完了を目標とする

#### 5.1.3 CLIの責務

- Hook設定の生成・インストール・アンインストール（既存設定を破壊しない）
- Daemonへのイベント送信
- 現在状態の表示（`status`, `list`）
- 手動状態変更（`event`, `reset`）
- 診断（`doctor`）
- テーマ確認・適用（`theme`）
- ログ確認（`logs`）

### 5.2 Provider Adapter

Provider Adapter は AIツール固有イベントを共通形式（Normalized Event）へ変換する。

#### 5.2.1 Provider インターフェース

```rust
pub trait ProviderAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn parse(&self, input: Value) -> Result<Vec<NormalizedEvent>, ProviderError>;
    fn validate(&self, input: &Value) -> ValidationResult;
    fn derive_session(&self, input: &Value) -> SessionIdentity;
}
```

#### 5.2.2 Provider 要件

- 不明なフィールドを無視できる（forward-compatible）
- 必須フィールド欠落時にクラッシュせず、検証エラーイベントへ変換する
- Provider名・バージョンをイベントへ記録する
- 未知イベントは `provider.schema_error` またはメタデータ付きの `unknown` イベントとしてログ記録する
- ツールのアップデートによる形式変更を検知できるよう、スキーマバージョンを保持する

#### 5.2.3 MVP対応Provider

| Provider | 対応 | 備考 |
|----------|------|------|
| `claude` | MVP正式対応 | Claude Code Hooks 経由 |
| `opencode` | Phase 2 | プラグイン/ローカルAPIから選択 |
| `codex` | Experimental | Completion通知のみ、公式仕様安定後の正式化 |
| `generic` | Phase 3 | Webhook/Socket API |

### 5.3 Local Event Broker（Daemon）

#### 5.3.1 通信方式

Unix Domain Socket（SOCK_STREAM）。

- 推奨パス: `$XDG_RUNTIME_DIR/agent-term-status.sock`
- フォールバック: `~/.local/state/agent-term-status/status.sock`

採用理由: TCPポート競合回避、外部ネットワーク非露出、低レイテンシー、ファイル権限によるアクセス制御、Hookからの短時間送信。

#### 5.3.2 プロトコル

- フレーミング: Length-Prefixed JSON（4バイトビッグエンディアン長 + JSONペイロード）
- エンコーディング: UTF-8
- イベントサイズ上限: **64 KiB/event**（超過は拒否、メタデータのみ記録）
- 認証: Unix Socket のファイルモード `0600`（ユーザー専用）

#### 5.3.3 Broker 機能

- Session Registry: セッションの登録・存活確認
- Event Validation: スキーマ検証
- Deduplication: 同一 `event_id` の重複排除
- 順序補正: タイムスタンプ逆転時の整合性維持

#### 5.3.4 Daemonライフサイクル

- シグナル処理: `SIGTERM`, `SIGINT` で安全に終了（保留中のRenderer出力をフラッシュ）
- ターミナル消失・tmuxペイン消失・Socket切断をイベント経由で検知しリセット
- Crash Recovery: 再起動後に残留状態を表示せず、セッション存在確認 → 不明はUNKNOWN経由でIDLEへ

### 5.4 State Engine

State Engine は Normalized Event を消费して Agent State を生成する。

#### 5.4.1 責務

- 状態遷移の判定（セクション8）
- 複数イベント間の優先順位解決（セクション8.2）
- TTL・タイムアウト処理（セクション8.4）
- Parent/Subagent 集約（セクション8.5）
- 通知抑制（デバウンス・フォーカス判定・Quiet Hours）

#### 5.4.2 State Engine不変条件

- 同一 `event_id` の再適用は冪等（状態・通知の重複発生なし）
- 不正・欠損イベントは状態を崩さず `ERROR`/`UNKNOWN` へ遷移
- 常に「安全側」へ倒す（不明時は WORKING ではなく UNKNOWN）

### 5.5 Rendering Engine

#### 5.5.1 責務

- Capability Detection: 実行環境のターミナル能力を検知
- Theme Resolution: 現在のテーマ・状態から表現を解決
- Rate Limiting: Renderer ごとの描画レート制限
- Feedback: Renderer失敗時に `renderer.failed` イベントを生成

#### 5.5.2 Renderer インターフェース

```rust
#[async_trait]
pub trait Renderer: Send + Sync {
    async fn detect(&self, ctx: &TerminalContext) -> Result<RendererCapabilities>;
    async fn render(&self, view: &StateView) -> Result<()>;
    async fn reset(&self, target: &RenderTarget) -> Result<()>;
    async fn health_check(&self) -> HealthStatus;
}
```

---

## 6. データモデル

### 6.1 Normalized Event

```json
{
  "schema_version": "1.0",
  "event_id": "018f2b70-5f14-7fb7-a880-123456789abc",
  "timestamp": "2026-07-18T07:15:31.123Z",
  "provider": "claude",
  "provider_version": "1.0",
  "event_type": "tool.started",
  "session": {
    "id": "provider-session-id",
    "parent_id": null,
    "workspace": "/Users/user/src/project",
    "terminal": {
      "tty": "/dev/ttys004",
      "term_program": "iTerm.app",
      "tmux_pane": "%12"
    }
  },
  "activity": {
    "category": "shell",
    "label": "Running tests",
    "tool_name": "Bash",
    "risk": "low"
  },
  "metadata": {}
}
```

#### 6.1.1 必須フィールド

- `schema_version`
- `event_id`（UUIDv7推奨・時系列ソート可能）
- `timestamp`（RFC 3339, UTC）
- `provider`
- `event_type`
- `session.id`

#### 6.1.2 情報最小化（プライバシー）

以下は標準で収集・保存しない。

- プロンプト本文・AI応答本文
- ファイル内容・コマンド全文・ファイルパス全体
- APIキー・環境変数・ユーザー名
- リポジトリのリモートURL

コマンド分類はメモリ上でのみ行い、分類結果（カテゴリ・リスク）のみを保持する。ファイルパスはbasenameのみ（`store_workspace_paths: false` 既定）。

### 6.2 Agent State

| 状態 | 意味 | ユーザー行動 |
|------|------|--------------|
| `IDLE` | セッション待機中 | 不要 |
| `WORKING` | AIが処理中 | 不要 |
| `ATTENTION` | 入力または承認待ち | 必要 |
| `RISK` | 高リスク操作の確認待ち | 即時確認 |
| `RESULT` | 正常完了 | 任意 |
| `ERROR` | 失敗または連携異常 | 確認推奨 |
| `UNKNOWN` | 状態判定不能 | 状況確認 |

### 6.3 Activity Model

Activity は状態に付随する短いラベル（例: `WORKING · Running tests`）。状態色の種類を増やす理由にはしない（"テスト実行"も"ファイル編集"も WORKING）。

#### 6.3.1 Activity Category

`thinking`, `reading`, `searching`, `editing`, `shell`, `testing`, `network`, `version_control`, `deployment`, `unknown`

#### 6.3.2 Activity Label 制約

- 40文字以内
- 秘密情報を含めない
- コマンド全文を表示しない
- ファイルパスは標準でbasenameのみ
- 制御文字を必ず除去
- Provider由来の未検証文字列をそのまま制御シーケンスへ渡さない（サニタイズ必須）

### 6.4 Session Identity

#### 6.4.1 識別キー優先順位

1. Provider 提供の session ID
2. tmux pane ID（`TMUX_PANE`）
3. TTY（`/dev/ttysXXX`）
4. terminal session ID
5. workspace + PID の組み合わせ

#### 6.4.2 tmux対応

tmux 内ではターミナル全体ではなく対象ペインのみを更新する。`TMUX_PANE=%12` をセッションターゲットとして使用。

#### 6.4.3 サブエージェント

```
Parent Session
 ├─ Subagent A: WORKING
 ├─ Subagent B: ATTENTION
 └─ Subagent C: RESULT
```

- 親セッションの表示は子の最優先状態を集約
- Provider が親子を識別できない場合は無理に推測せず同一セッションとして扱う

---

## 7. イベントタイプ

### 7.1 セッションイベント

- `session.started`
- `session.stopped`
- `session.failed`
- `session.heartbeat`

### 7.2 AI処理イベント

- `agent.started`
- `agent.working`
- `agent.waiting`
- `agent.completed`
- `agent.failed`

### 7.3 ツールイベント

- `tool.started`
- `tool.completed`
- `tool.failed`

### 7.4 ユーザー操作イベント

- `user.input_required`
- `user.approval_required`
- `user.input_received`

### 7.5 システムイベント

- `provider.disconnected`
- `provider.schema_error`
- `renderer.failed`
- `session.timeout`

---

## 8. 状態遷移とTTL

### 8.1 状態遷移図

```
            ┌───────────┐
            │   IDLE    │
            └─────┬─────┘
                  │ agent.started
                  ▼
            ┌───────────┐
       ┌────│  WORKING  │◀────────────┐
       │    └─────┬─────┘             │
       │          │ completed         │ input_received
approval         │                   │
required         ▼                   │
       ┌─────────────┐  ┌─────────────┐
       │  ATTENTION  │  │   RESULT    │
       └──────┬──────┘  └──────┬──────┘
              │                 │ timeout
              └─────────────────┴──────┐
                                     │
任意状態 ── failed ──▶ ERROR          │
ATTENTION ── high-risk ──▶ RISK      │
                                     │
TTL経過 ──▶ UNKNOWN ──▶ IDLE ◀───────┘
```

### 8.2 表示優先順位

複数イベントが同時に存在する場合:

```
RISK > ATTENTION > ERROR > RESULT > WORKING > IDLE > UNKNOWN
```

**例外**: ERROR が Provider連携エラーであり AI本体は ATTENTION 状態の場合は、ATTENTION を優先する。

### 8.3 RISK状態の遷移

- `ATTENTION` 状態で高リスク承認要求を検知した場合に `RISK` へ遷移
- MVP UIでは RISK を ATTENTION の強調表現として取り扱う（色・アイコンの強度のみ変更）
- ユーザー承認後は `WORKING` へ戻る

### 8.4 State TTL

| 状態 | 標準TTL |
|------|---------|
| WORKING | 30分 |
| ATTENTION | 4時間 |
| RISK | 30分 |
| RESULT | 8秒 |
| ERROR | 60秒 |
| UNKNOWN | 30秒 |

TTL経過後は即座に IDLE ではなく **一度 UNKNOWN** へ遷移させる。UNKNOWNのTTL経過後に IDLE へ戻る。

### 8.5 親子セッション集約

- 親セッションの表示状態 = 子セッション状態の最優先
- 子が1つでも ATTENTION 以上なら親も ATTENTION 以上
- 子のTTL・タイムアウトは子ごとに管理

### 8.6 復旧機構

- **Heartbeat**: Providerが対応する場合、処理中に一定間隔で `session.heartbeat` を送信。MVPではClaude Code Hook単体では完全なheartbeatが得られないため、プロセス存在確認を補助的に利用する
- **Shell Prompt Reset**: シェルプロンプト再表示時に一定条件で状態を復旧するShell Integrationをオプション提供
- **Crash Recovery**: Daemon再起動後、残留状態は再表示せずセッション存在確認 → 不明はUNKNOWN経由でIDLE

### 8.7 通知抑制

#### 8.7.1 デバウンス標準値

- 同一状態の再描画: **250ms**
- 同一通知の再送: **10秒**
- RESULT通知の再送: **30秒**

#### 8.7.2 フォーカス判定

- 対象ターミナルがフォーカスされている場合、ATTENTION と RESULT のmacOS通知を抑制できる
- **RISKはフォーカス中でも通知する**

#### 8.7.3 Quiet Hours

```yaml
notifications:
  quiet_hours:
    start: "22:00"
    end: "07:00"
    allow:
      - risk
```

---

## 9. Provider: Claude Code 統合

### 9.1 Hookイベントマッピング

| Claude Codeイベント | Normalized Event | Agent State |
|--------------------|------------------|----------------|
| `SessionStart` | `session.started` | IDLE |
| `UserPromptSubmit` | `agent.started` | WORKING |
| `PreToolUse` | `tool.started` | WORKING |
| `PostToolUse` | `tool.completed` | WORKING |
| `PostToolUseFailure` | `tool.failed` | ERROR または WORKING（内容による） |
| `Notification` | `user.*`（内容により分類） | ATTENTION |
| `Stop` | `agent.completed` | RESULT |
| `SessionEnd` | `session.stopped` | IDLE |

実際のマッピングはイベント名だけでなく、入力JSONの内容と Claude Code バージョンに基づいて決定する。

### 9.2 Hook コマンド

```
ats ingest --provider claude
```

- JSONは標準入力から受け取る
- stdoutを汚染しない（何も出力しない、またはClaude Codeが無視できる形式）
- エラー時も終了コード0を返す設定（`--allow-failure` 相当）をデフォルト化し、Claude Code本体の処理を妨げない

```
Claude Code
   │ JSON stdin
   ▼
ats ingest --provider claude
   │ normalized event
   ▼
Unix Domain Socket
   │
   ▼
ats daemon
```

### 9.3 インストール方式

```
ats install claude --scope user      # ~/.claude/settings.json
ats install claude --scope project   # ./.claude/settings.json
ats install claude --scope local     # ./.claude/settings.local.json
```

- 既存設定を破壊せず、設定ファイルを構文解析して必要なHookだけ追加する
- agent-term-statusが挿入したエントリにはマーカー（コメントまたは専用フィールド）を付与し、アンインストール時に当該エントリのみ削除する
- インストール前に `--dry-run` で差分表示を行える
- 設定変更前にバックアップを作成する（`.bak` または世代管理）

---

## 10. Renderer仕様

### 10.1 Renderer Capability Model

```rust
pub struct RendererCapabilities {
    pub background: bool,
    pub tab_title: bool,
    pub tab_color: bool,
    pub badge: bool,
    pub cursor_color: bool,
    pub pane_border: bool,
    pub notification: bool,
    pub flash: bool,
    pub reset_reliable: bool,
}
```

### 10.2 MVP Renderer

#### 10.2.1 iTerm2 Renderer

| 機能 | MVP対応 | 備考 |
|------|---------|------|
| セッションプロファイル切り替え | ○（オプション） | 既定プロファイルと状態別プロファイルの切り替え |
| タブ・ウィンドウタイトル | ○ | OSC 0/2 |
| バッジ | ○ | OSC 1337 SetBadgeFormat |
| 背景色 | △（オプション・既定OFF） | 暗色のわずかな変更のみ推奨 |
| 通知 | ○ | macOS Notification Renderer経由 |

#### 10.2.2 tmux Renderer

| 機能 | MVP対応 | 備考 |
|------|---------|------|
| ペイン枠色 | ○ | `pane-border-style` / `pane-active-border-style` |
| ペインタイトル | ○ | `pane-border-status` + `pane-border-format` |
| ウィンドウステータス | △ | オプション |
| ステータスバー | △ | オプション |

#### 10.2.3 macOS Notification Renderer

通知対象状態:

- `ATTENTION`（標準通知）
- `RISK`（強い通知・表示保持）
- `RESULT`（任意・既定ON）
- `ERROR`（標準通知）

`WORKING` では通知しない。通知にはOSの通知センターを利用し、`osascript` 依存を避ける（ネイティブAPIまたは安定した通知ライブラリを使用）。

#### 10.2.4 今後のRenderer

- **OSC Renderer**: 汎用の制御シーケンスRenderer（Phase 2+）
- **WezTerm / Ghostty**: 動的カラーパレット変更対応（Phase 2+）

### 10.3 表示の原則

- 色だけに依存せず、各状態は最低2種類の表現を使用（色＋テキストラベル、色＋アイコン、など）
- ターミナル本来の表示を破壊しない（背景色は既定OFF・オプション）
- ANSIカラーとのコントラスト、Vim/TUIアプリの配色、SSH接続先警告色との競合を避ける
- リセット失敗時に色が残らないよう、`reset_reliable` が偽のRendererではフォールバック表現を使用

---

## 11. テーマ仕様

### 11.1 標準表現

| 状態 | 色 | アイコン | 通知 |
|------|----|----------|------|
| IDLE | デフォルト | `○` | なし |
| WORKING | 青 | `●` | なし |
| ATTENTION | オレンジ | `!` | あり |
| RISK | 赤 | `!!` | 強い通知 |
| RESULT | 緑 | `+` | 任意 |
| ERROR | マゼンタ または 暗赤 | `×` | あり |
| UNKNOWN | グレー | `?` | 原則なし |

絵文字の表示幅問題を避けるため、**ASCIIアイコンセット** を標準とし、絵文字セットをオプションで提供する。

### 11.2 同梱テーマ

- `default`
- `color-safe`（色覚対応）
- `low-distraction`（控えめ）
- `high-contrast`
- `monochrome-symbols`

### 11.3 背景色モード

```yaml
rendering:
  background:
    enabled: false        # 既定OFF
    intensity: subtle     # subtle | medium | strong
```

有効化時も完全な単色背景ではなく、暗い色調のわずかな変更を推奨する。

---

## 12. 設定ファイル仕様

### 12.1 パス

- ユーザー設定: `~/.config/agent-term-status/config.yaml`
- プロジェクト設定: `./.agent-term-status/config.yaml`（オプション・ユーザー設定を上書き）

設定は一時ファイルへ書き込み、検証後にatomic renameする。

### 12.2 スキーマ

```yaml
version: 1

daemon:
  enabled: true
  log_level: warn         # trace | debug | info | warn | error
  event_retention: 24h
  socket_path: null       # null = 自動検出

privacy:
  store_activity_labels: false
  store_workspace_paths: false
  redact_home_directory: true

states:
  working:
    color: "#2457A6"
    label: "Working"
    symbol: "*"
  attention:
    color: "#D97706"
    label: "Needs input"
    symbol: "!"
  risk:
    color: "#B91C1C"
    label: "Risk"
    symbol: "!!"
  result:
    color: "#15803D"
    label: "Completed"
    symbol: "+"
  error:
    color: "#9333EA"
    label: "Error"
    symbol: "x"
  unknown:
    color: "#6B7280"
    label: "Unknown"
    symbol: "?"

rendering:
  background:
    enabled: false
    intensity: subtle

renderers:
  tmux:
    enabled: auto         # auto | on | off
    pane_border: true
    pane_title: true
  iterm2:
    enabled: auto
    badge: true
    tab_title: true
    background: false
  notifications:
    enabled: true
    states:
      - attention
      - risk
      - result
      - error
    quiet_hours:
      start: "22:00"
      end: "07:00"
      allow:
        - risk

providers:
  claude:
    enabled: true
  opencode:
    enabled: false
  codex:
    enabled: false
    experimental: true

tts:
  enabled: false
  states: []

logging:
  level: warn
  retention: 7d
  redact: true
```

---

## 13. リスク分類

### 13.1 MVPでの扱い

- MVPの RISK は、AIツールが承認を要求したイベントのうち **明示的に高リスクと判断できる場合のみ** 使用
- agent-term-status 自身は危険操作をブロックしない
- 安全機構としては宣伝せず、「承認要求を目立たせる」機能として扱う

### 13.2 リスクレベル

`low`, `medium`, `high`, `critical`, `unknown`

### 13.3 高リスク候補（参考）

- 本番環境へのデプロイ
- `git push --force`, `git reset --hard`
- 大量ファイル削除
- `terraform apply`
- `kubectl delete`
- データベース破壊操作
- パッケージ公開
- 外部へのデータ送信

### 13.4 設計上の制約

コマンド文字列の正規表現だけでのRISK判定は信頼性が低い（`echo "kubectl delete"` の誤検知、ラッパースクリプト内の隠蔽された危険操作の見逃し）。MVPではリスク分類を **安全機構ではなく注意喚起** として扱い、将来的なポリシーエンジンは別コンポーネント（Phase 3+）とする。

---

## 14. セキュリティ要件

### 14.1 脅威モデル

- Hook入力に悪意ある文字列・制御シーケンスが含まれる
- 他プロセスが偽イベントを送信する
- Socket権限が不適切
- ログへ機密情報が残留する
- 設定インストール時に既存Hookを破壊する
- agent-term-statusが高権限コマンドを実行する

### 14.2 対策

- 受信JSONの厳格なスキーマ検証
- 表示文字列から制御文字（C0/C1/DEL/サロゲート単独等）を削除
- Socket権限を `0600`（ユーザーのみ）へ制限
- イベントサイズ上限 **64 KiB**
- Hook入力をシェル評価しない
- コマンド文字列を再実行しない
- デフォルトで本文をログ保存しない（`logging.redact: true`）
- 設定変更前にバックアップを作成し、atomic renameで更新
- `install` 時に差分表示（`--dry-run`）
- Rendererは許可された制御シーケンスのみ生成（Allowlist方式）

### 14.3 権限原則

agent-term-statusはユーザー権限で動作し、`sudo` 等の権限昇格を行わない。ファイル権限はユーザー読み書き専用（`0600`）を標準とする。

---

## 15. 信頼性要件

### 15.1 Fail-open

agent-term-statusが停止・クラッシュしていても、AIエージェントの処理は継続する。Hookコマンドはエラー時も終了コード0で復帰できる設定を既定とする。

### 15.2 Idempotency

同一 `event_id` を複数回受信しても、状態遷移と通知を重複させない。

### 15.3 Atomic Configuration

設定更新は一時ファイルへ書き込み、検証後にatomic renameする。

### 15.4 Crash Recovery

Daemon再起動後、残留状態を再表示せず、セッション存在確認を行う。確認できない状態はUNKNOWN経由でIDLEへ戻す。

### 15.5 Reset Guarantee

```
ats reset --all
```

ですべての表示を強制的に既定状態へ戻せる。

---

## 16. パフォーマンス要件

| 項目 | 目標 |
|------|------|
| Hook CLI実行時間（`ats ingest`） | 50ms以下 |
| イベント反映時間（Hook → 表示） | 150ms以下 |
| Daemon常駐メモリ | 30MB以下 |
| アイドルCPU使用率 | 0.1%未満 |
| イベントスループット | 100 events/sec 以上 |
| 起動時間（Daemon） | 300ms以下 |

実装言語（Rust）とRenderer方式に応じて調整するが、Hook遅延とアイドルリソース消費は最優先で守る。

---

## 17. 技術スタック

### 17.1 採用技術（確定）

| 用途 | 技術 |
|------|------|
| 実装言語 | Rust（最新安定版） |
| 非同期ランタイム | Tokio |
| CLIパーサー | clap |
| シリアライズ | serde / serde_json / serde_yaml |
| ローカル通信 | Unix Domain Socket（Tokio） |
| ログ | tracing / tracing-subscriber |
| UUID生成 | uuid（UUIDv7） |
| macOS通知 | ネイティブAPI（UserNotifications）または安定ライブラリ。`osascript` には依存しない |
| 配布 | Homebrew Tap / GitHub Releases / cargo install |

### 17.2 Rust採用理由

- 単一バイナリ配布が容易
- 常駐プロセスの低メモリ化
- Unix Socket・シグナル処理との親和性
- シェルHookからの高速起動（起動時間300ms以下）
- 入力検証を型で表現しやすい
- クロスプラットフォーム展開の余地（Linux版はPhase 2以降）

### 17.3 代替案の扱い

技術スパイク（Phase 0）ではTypeScript・Pythonでの検証も許容する。ただし製品版はRustとし、PythonはCLI起動時間・依存関係・配布・常駐プロセス管理の観点から探用しない。

---

## 18. ディレクトリ・パッケージ構成

### 18.1 リポジトリ構成

```
agent-term-status/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── ats-core/                 # Normalized Event, Agent State, Session Identity
│   ├── ats-cli/                  # CLI (bin: ats, agent-term-status)
│   ├── ats-daemon/               # Daemon (bin: ats-daemon)
│   ├── ats-provider-claude/      # Claude Code Provider
│   ├── ats-state-engine/         # State Engine, TTL, Aggregation
│   ├── ats-renderer-tmux/        # tmux Renderer
│   ├── ats-renderer-iterm2/      # iTerm2 Renderer
│   ├── ats-renderer-notification/# macOS Notification Renderer
│   └── ats-config/               # 設定・テーマ読み込み
├── schemas/
│   └── event-v1.schema.json      # Normalized Event JSON Schema
├── themes/
│   ├── default.yaml
│   ├── color-safe.yaml
│   ├── low-distraction.yaml
│   ├── high-contrast.yaml
│   └── monochrome-symbols.yaml
├── integrations/
│   └── claude/                   # Claude Code Hook設定テンプレート
├── docs/
│   ├── IDEA.md
│   └── SPEC.md                   # 本書
├── tests/
│   ├── integration/
│   └── fixtures/
└── README.md
```

### 18.2 ランタイムパス

| 用途 | パス |
|------|------|
| Unix Socket（推奨） | `$XDG_RUNTIME_DIR/agent-term-status.sock` |
| Unix Socket（フォールバック） | `~/.local/state/agent-term-status/status.sock` |
| ユーザー設定 | `~/.config/agent-term-status/config.yaml` |
| ユーザーテーマ | `~/.config/agent-term-status/themes/` |
| ログ | `~/.local/state/agent-term-status/logs/` |
| イベント保持（任意） | `~/.local/state/agent-term-status/events.db`（設定時のみ） |
| PIDファイル | `$XDG_RUNTIME_DIR/agent-term-status.pid` |

XDG規格に従い、環境変数 `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_RUNTIME_DIR` を尊重する。

### 18.3 クレート命名規則

- ライブラリクレート: `ats-*`
- バイナリクレート: `ats-cli`（bin名 `ats` / `agent-term-status`）, `ats-daemon`（bin名 `ats-daemon`）
- `ats` は "Agent Terminal Status" の略

---

## 19. CLIリファレンス

### 19.1 install

```
ats install <provider> [--scope user|project|local] [--dry-run]
```

指定ProviderのHookを設定ファイルへ挿入する。`--dry-run` で差分表示のみ。

### 19.2 uninstall

```
ats uninstall <provider> [--scope user|project|local]
```

agent-term-statusが挿入したエントリのみ削除する。

### 19.3 ingest

```
ats ingest --provider <name>
```

標準入力からProvider形式のJSONを受け取り、Normalized Eventへ変換してDaemonへ送信する。Hookから呼ばれるコマンド。

### 19.4 event

```
ats event <state> [--activity <label>] [--session <id>]
```

手動でAgent Stateを送信する。デバッグ・テスト用。

### 19.5 reset

```
ats reset [--all | --session <id>]
```

表示状態をリセットする。`--all` ですべてのセッション表示を既定状態へ戻す。

### 19.6 status / list

```
ats status [--session <id>]
ats list [--json]
```

`status`: 現在フォアグラウンドセッションの状態表示。`list`: 全セッション一覧。

### 19.7 doctor

```
ats doctor
```

設定診断。以下を検査する。

- Daemon起動状態・Socket到達性
- Claude Code設定ファイルの構文・マーカー
- tmux/iTerm2検出・Capability
- 権限・パス
- 設定ファイルのスキーマ妥当性

### 19.8 theme

```
ats theme list
ats theme preview <name>
ats theme apply <name>
```

### 19.9 daemon

```
ats daemon start [--foreground]
ats daemon stop
ats daemon status
```

### 19.10 logs

```
ats logs [--tail] [--level <level>]
```

---

## 20. テスト要件

### 20.1 Unit Test

- Providerイベント変換（正常・欠損・不正）
- 状態遷移・優先順位
- TTL処理
- 重複イベント排除
- 制御文字除去・サニタイズ
- リスク分類
- テーマ解決
- 設定バリデーション

### 20.2 Integration Test

- Hook JSON → tmux表示までのE2E
- Daemon停止時のfail-open
- Socket権限
- 設定インストール・復元（既存設定保全）
- 複数セッションの分離（他ペインへ漏れない）
- イベント順序逆転
- Hook欠落時のタイムアウト
- ターミナル終了時のリセット

### 20.3 End-to-End Test

```
Claude Code開始
  → WORKING表示
  → ツール実行
  → WORKING + Activity更新
  → 承認待ち
  → ATTENTION + 通知
  → ユーザー承認
  → WORKING
  → 完了
  → RESULT
  → 8秒後にIDLE
```

### 20.4 Fault Injection

- Daemon強制終了
- Hook入力の途中切断
- 不正JSON送信
- 1000イベント連続送信
- tmux/iTerm2の途中終了
- 古いProviderスキーマ送信
- タイムスタンプ逆転イベント送信
- Socket権限の変更

---

## 21. MVP受け入れ条件

以下を **すべて** 満たした場合、MVP完成とする。

1. `ats install claude` で既存設定を壊さずHookを追加できる
2. Claude Code開始後に対象ペインが `WORKING` になる
3. ユーザー入力・承認待ちを `ATTENTION` として表示できる
4. 完了時に `RESULT` を表示し、一定時間後に元へ戻る
5. tmuxの別ペインへ状態が漏れない
6. agent-term-status停止中でもClaude Codeが正常動作する（fail-open）
7. 強制終了後も表示状態を手動リセットできる（`ats reset --all`）
8. Hook入力内容が標準では永続保存されない
9. `ats doctor` が主要な設定不良を検出する
10. 30分の連続利用で明らかな状態残留が発生しない

---

## 22. 開発フェーズ

### Phase 0: 技術スパイク

技術的成立条件を満たした時点で終了（期間は問わない）。

- Claude Code Hookからイベント取得
- tmuxペイン特定
- ペイン枠色変更
- 完了後の復旧

成果物（デーモンなし・standalone）:

```
ats event working
ats event attention
ats event result
```

### Phase 1: MVP 0.1

本仕様書のスコープ（セクション3.1）を実装する。

- Claude Code Provider
- State Engine + Daemon
- tmux / iTerm2 / Notification Renderer
- `install` / `doctor` / `reset` / `status` / `list`
- 標準テーマ（5種）
- Homebrew配布

### Phase 2: 複数エージェント対応

- OpenCode Provider
- 複数セッション一覧UIの強化
- サブエージェント集約の実運用化
- WezTerm / Ghostty Renderer
- テーマ共有機能

### Phase 3: 拡張基盤

- Provider SDK / Renderer SDK
- Generic Webhook / Socket API（`generic-provider`）
- VS Code / Cursor 連携
- メニューバーアプリ
- チーム向け設定配布
- ポリシーエンジン（RISK判定の分離）

---

## 23. 成功指標

ダウンロード数より継続利用を重視する。

### 23.1 プロダクト指標

- インストール成功率: **90%以上**
- 7日後継続利用率: **30%以上**
- 1日あたりのATTENTION検出数
- 状態残留エラー率: **1%未満**
- 手動 `reset` 実行率
- 通知無効化率
- アンインストール理由

### 23.2 検証仮説

1. ユーザーは背景色よりペイン枠を好むか
2. 完了通知より入力待ち通知に価値を感じるか
3. 単一エージェント利用者にも価値があるか
4. 状態表示は5種類で十分か
5. agent-term-statusのためにtmuxを導入する利用者がいるか
6. iTerm2単体ユーザーにも同等の価値を提供できるか

---

## 24. MVP定義サマリ

> Claude Codeを複数のtmuxペインで実行している開発者が、作業中・入力待ち・完了・異常を、ペインを開かずに識別できるローカルCLIツール。

```
Claude Code Hooks
    ↓
ats ingest --provider claude
    ↓
Local Daemon (Unix Socket)
    ↓
State Engine
    ↓
tmux pane border + title
    ↓
iTerm2 badge + macOS notification
```

**MVP標準状態**: `IDLE`, `WORKING`, `ATTENTION`, `RESULT`, `ERROR`

`RISK` は内部仕様として準備するが、初期UIでは `ATTENTION` の強調表示として扱う。

---

## 付録 A: プロダクト名とコマンド名の決定

### A.1 プロジェクト名

- リポジトリ名・配布名: **`agent-term-status`**
- 表示名（ドキュメント・宣伝）: "Agent Terminal Status" または "agent-term-status"

IDEA.md の「Agent Presence」は短く分かりやすいが、一般語 "presence" との衝突・商標リスクを避けるため、`agent-term-status` を正式名とする。

### A.2 コマンド名

- フルネーム: `agent-term-status`
- 推奨短縮名: **`ats`**
- `presence` は一般語衝突のため使用しない

### A.3 パッケージ・クレート名

- crates: `ats-*`
- Homebrew formula: `agent-term-status`（`ats` バイナリを提供）
- cargo install: `agent-term-status`

---

## 付録 B: IDEA.mdからの主な変更点

| 項目 | IDEA.md | SPEC.md | 理由 |
|------|---------|---------|------|
| プロジェクト名 | Agent Presence | agent-term-status | ユーザー指示・一般語衝突回避 |
| コマンド名 | `presence`（仮称） | `ats` / `agent-term-status` | 一般語衝突回避 |
| Socketパス | agent-presence.sock | agent-term-status.sock | プロジェクト名統一 |
| 設定ディレクトリ | （明示なし） | `~/.config/agent-term-status/` | XDG規格準拠 |
| クレート構成 | presence-* | ats-* | プロジェクト名統一 |
| 通知実装 | osascript依存を避ける（候補） | ネイティブAPI必須 | 安定性確保 |
| TTL経過後 | UNKNOWNへ | UNKNOWN経由でIDLE | より明確な復路定義 |
| RISKの扱い | 内部仕様（MVP定義に記載） | MVPではATTENTION強調表示 | 統一 |

---

## 改訂履歴

| バージョン | 日付 | 変更 |
|------------|------|------|
| 1.0 | 2026-07-18 | 初版。IDEA.md v1.0 Draftをベースに仕様として確定（プロジェクト名: agent-term-presence） |
| 1.1 | 2026-07-18 | プロジェクト名を `agent-term-status` へ変更。それに伴いコマンド短縮名 `atp`→`ats`、クレート `atp-*`→`ats-*`、Socket `presence.sock`→`status.sock`、内部概念 `Presence State`→`Agent State`、`PresenceView`→`StateView` へ一括更新 |
