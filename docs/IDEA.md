Agent Presence

AIコーディングエージェント状態可視化ツール 機能仕様書

文書バージョン: 1.0 Draft
対象バージョン: MVP 0.1
対象OS: macOS
対象ユーザー: CLI型AIコーディングエージェントを並列利用するソフトウェア開発者

⸻

1. エグゼクティブサマリー

Agent Presenceは、Claude Code、OpenCode、Codex CLIなどのAIコーディングエージェントが現在どのような状態にあるかを、ターミナルの色、タブ、ペイン枠、バッジ、通知によって視覚化するローカルアプリケーションである。

本製品が解決する中心課題は、AI内部の処理状況を詳細に表示することではない。

中心課題は次の問いに即座に答えることである。

ユーザーは、今このターミナルに注意を向ける必要があるか。

そのため、AIツール固有の細かなイベントを、以下の少数の「ユーザー行動状態」に正規化する。

* IDLE：操作不要
* WORKING：AIが作業中
* ATTENTION：ユーザーの入力または承認が必要
* RISK：危険度の高い操作を実行しようとしている
* RESULT：処理が完了した
* ERROR：処理に失敗した、または連携が壊れている

MVPではClaude Code、iTerm2、tmuxに対応する。OpenCodeは次期対応候補とし、Codex CLIはHook仕様の安定性を確認しながら実験的対応とする。

Claude Codeはライフサイクルの複数地点でシェルコマンドやHTTPエンドポイントを実行でき、Hook入力としてJSONを渡せるため、最初の統合対象に適している。OpenCodeもプラグインからイベントへ接続できる。一方、CodexのHook機能は2026年時点で変更や不具合の報告があるため、正式対応を急ぐと保守負担が大きくなる可能性がある。(Claude Platform Docs)

⸻

2. 背景と課題

AIコーディングエージェントの処理時間が長くなると、ユーザーは複数のターミナル、リポジトリ、worktree、エージェントを同時に扱うようになる。

その結果、以下の問題が発生する。

2.1 状態確認のための巡回

ユーザーは各ターミナルを順番に開き、次の状態を確認する必要がある。

* AIがまだ処理中か
* 入力待ちになっていないか
* 権限承認を要求していないか
* タスクが完了していないか
* エラーで停止していないか

これはAIによって削減された作業時間の一部を、監視作業によって再び消費する。

2.2 小さな文字表示では気づきにくい

ステータスラインやターミナルタイトルに状態を表示しても、複数ウィンドウを縮小表示している場合には判別しにくい。

色、枠、点滅、バッジなどのアンビエントな表現であれば、ユーザーは文字を読まずに状態を認識できる。

2.3 AIツールごとにイベント形式が異なる

各AIツールでは、Hook名、イベント粒度、入力形式、設定方法が異なる。

したがって、ターミナル表示側がClaude Code、OpenCode、Codex CLIの仕様を直接理解する設計では、ツールごとの変更に強く依存する。

⸻

3. プロダクトビジョン

複数のAIエージェントの中から、人間の注意を必要としているセッションを一目で特定できる状態表示レイヤーを提供する。

Agent PresenceはAIの思考過程を正確に再現するものではない。

AIイベントを、人間にとって有用な状態へ変換する。

AIツール固有イベント
        ↓
イベント正規化
        ↓
セッション状態判定
        ↓
ユーザー注意状態
        ↓
ターミナル表現

⸻

4. スコープ

4.1 MVPに含める機能

* Claude Code Hookとの連携
* セッション単位の状態管理
* iTerm2のタブまたはセッション表示変更
* tmuxペイン枠およびペインタイトル変更
* macOS通知
* YAMLによるテーマ設定
* 状態の自動復旧
* CLIによる手動イベント送信
* doctorコマンドによる設定診断
* Hook設定の自動インストール
* ローカル動作
* ログの保存期間および無効化設定

4.2 MVPに含めない機能

* AIの思考内容の解析
* プロンプトやコード内容のクラウド送信
* AIエージェント自体の実行
* tmuxやターミナルエミュレータの代替
* 複数端末間の同期
* Webダッシュボード
* チーム管理
* 危険コマンドの強制ブロック
* AI利用料金やトークン量の集計
* WindowsおよびLinuxの正式対応
* Codex CLIの正式サポート

⸻

5. 設計原則

5.1 内部処理ではなくユーザー行動を表現する

thinking、planning、reading、searching、editingなどをすべて異なる色にすると、ユーザーが意味を覚えられない。

そのため、UI状態は最大6種類とする。

内部では詳細なイベントを保持できるが、表示上はユーザーの行動に基づいて集約する。

5.2 色だけに依存しない

各状態は最低2種類の表現を使用する。

例：

* 色
* テキストラベル
* アイコン
* ペイン枠
* タブタイトル
* macOS通知

これにより、色覚特性、低彩度ディスプレイ、背景テーマとの競合に対応する。

5.3 ターミナル本来の表示を破壊しない

背景全体の変更は視認性が高い一方で、以下の問題がある。

* ANSIカラーとのコントラストが変わる
* VimやTUIアプリの配色を壊す
* SSH接続先の警告色と競合する
* 長時間の強い色表示が疲労につながる
* リセット失敗時に色が残る

したがって、MVPの標準表現は以下とする。

1. tmuxペイン枠
2. タブタイトル
3. iTerm2バッジまたはプロファイル
4. macOS通知
5. 背景色はオプション

5.4 外部サービスを必要としない

すべてのイベント処理と状態保存はローカルで完結させる。

5.5 Hookが失敗してもAI作業を妨げない

表示ツールの障害によって、Claude Codeなどの本来の処理が失敗してはならない。

すべてのHook呼び出しは原則として次の条件を満たす。

* 短時間で終了する
* エラー時にも終了コード0を返せる設定を持つ
* 非同期処理に対応する
* 標準出力を汚染しない
* ロック待ちを発生させない

Claude CodeのHookはシェルコマンド、HTTP、プロンプト形式などを利用でき、非同期Hookも扱えるため、Agent Presenceでは非同期または短時間のローカル送信を基本とする。(Claude Platform Docs)

⸻

6. システムアーキテクチャ

┌─────────────────────────────────────────────┐
│ Event Sources                               │
│                                             │
│ Claude Code Hooks                           │
│ OpenCode Plugin             Future          │
│ Codex Hooks / Notify        Experimental    │
│ Manual CLI                                   │
│ Generic Shell Integration   Future          │
└──────────────────────┬──────────────────────┘
                       │ Native Event
                       ▼
┌─────────────────────────────────────────────┐
│ Provider Adapter                            │
│                                             │
│ claude-provider                             │
│ opencode-provider                           │
│ codex-provider                              │
│ generic-provider                            │
└──────────────────────┬──────────────────────┘
                       │ Normalized Event
                       ▼
┌─────────────────────────────────────────────┐
│ Local Event Broker                          │
│                                             │
│ Unix Domain Socket                          │
│ Session Registry                            │
│ Event Validation                            │
│ Deduplication                               │
└──────────────────────┬──────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────┐
│ State Engine                                │
│                                             │
│ State Machine                               │
│ Priority Resolution                         │
│ Timeout Processing                          │
│ Parent/Subagent Aggregation                 │
└──────────────────────┬──────────────────────┘
                       │ Presence State
                       ▼
┌─────────────────────────────────────────────┐
│ Rendering Engine                            │
│                                             │
│ Theme Resolution                            │
│ Capability Detection                        │
│ Rate Limiting                               │
└────────────┬────────────┬────────────┬───────┘
             │            │            │
             ▼            ▼            ▼
          iTerm2         tmux       Notification

⸻

7. コンポーネント仕様

7.1 CLI

実行ファイル名は仮称としてpresenceとする。

presence install claude
presence uninstall claude
presence event working
presence event attention
presence event result
presence reset
presence status
presence list
presence doctor
presence theme list
presence theme preview
presence daemon start
presence daemon stop

主な責務

* Hook設定の生成
* デーモンへのイベント送信
* 現在状態の表示
* 手動状態変更
* 診断
* テーマ確認
* ログ確認

CLIからのイベント送信は50ミリ秒以内で完了することを目標とする。

⸻

7.2 Provider Adapter

Provider Adapterは、AIツール固有イベントを共通形式に変換する。

Claude Code PreToolUse
        ↓
provider = claude
event_type = tool.started
activity = shell

OpenCodeはプラグインから各種イベントへ接続でき、SDKおよびサーバーAPIも提供しているため、将来的にはHookだけでなくプラグインまたはローカルAPIによる統合を選択できる。(OpenCode)

Providerインターフェース

interface ProviderAdapter {
  readonly name: string;
  parse(input: unknown): NormalizedEvent | NormalizedEvent[];
  validate(input: unknown): ValidationResult;
  deriveSession(input: unknown): SessionIdentity;
}

Provider要件

* 不明なフィールドを無視できる
* 必須フィールド欠落時にクラッシュしない
* Providerバージョンを記録できる
* 未知イベントをログへ記録できる
* ツールのアップデートによる形式変更を検知できる

⸻

7.3 Local Event Broker

イベントはUnix Domain Socketでローカルデーモンへ送信する。

候補パス：

$XDG_RUNTIME_DIR/agent-presence.sock

macOSで利用できない場合：

~/.local/state/agent-presence/presence.sock

Unix Domain Socketを採用する理由

* TCPポートの競合を避けられる
* 外部ネットワークへ露出しない
* 低レイテンシー
* ファイル権限でアクセス制御可能
* Hookから短時間で送信可能

フォールバック

デーモンが起動していない場合は、CLIが直接ターミナル表現を変更する「standalone mode」を提供する。

ただしstandalone modeでは、複数イベントの競合解決やタイムアウト処理は限定される。

⸻

8. 正規化イベントモデル

{
  "schema_version": "1.0",
  "event_id": "018f2b70-5f14-7fb7-a880-123456789abc",
  "timestamp": "2026-07-18T07:15:31.123Z",
  "provider": "claude",
  "provider_version": "unknown",
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

8.1 必須フィールド

* schema_version
* event_id
* timestamp
* provider
* event_type
* session.id

8.2 情報最小化

以下の情報は標準では保存しない。

* プロンプト本文
* ファイル内容
* コマンド全体
* AI応答本文
* APIキー
* 環境変数
* ユーザー名
* リポジトリのリモートURL

コマンドを分類する場合でも、原文を永続化せずメモリ上で評価し、分類結果だけを保持する。

⸻

9. イベントタイプ

9.1 セッションイベント

session.started
session.stopped
session.failed
session.heartbeat

9.2 AI処理イベント

agent.started
agent.working
agent.waiting
agent.completed
agent.failed

9.3 ツールイベント

tool.started
tool.completed
tool.failed

9.4 ユーザー操作イベント

user.input_required
user.approval_required
user.input_received

9.5 システムイベント

provider.disconnected
provider.schema_error
renderer.failed
session.timeout

⸻

10. Presence State

10.1 UI状態定義

状態	意味	ユーザー行動
IDLE	セッションは待機中	不要
WORKING	AIが処理中	不要
ATTENTION	入力または承認待ち	必要
RISK	高リスク操作の確認待ち	即時確認
RESULT	正常完了	任意
ERROR	失敗または連携異常	確認推奨
UNKNOWN	状態を判定不能	状況確認

10.2 表示優先順位

複数イベントが存在する場合、以下の優先順位を使用する。

RISK
  >
ATTENTION
  >
ERROR
  >
RESULT
  >
WORKING
  >
IDLE
  >
UNKNOWN

ただしERRORがProvider連携エラーであり、AI本体はATTENTION状態の場合は、ATTENTIONを優先する。

10.3 状態遷移

             ┌───────────┐
             │   IDLE    │
             └─────┬─────┘
                   │ agent.started
                   ▼
             ┌───────────┐
        ┌────│  WORKING  │◀────────────┐
        │    └─────┬─────┘             │
        │          │                   │
approval│          │ completed         │ input_received
required│          │                   │
        ▼          ▼                   │
┌─────────────┐  ┌─────────────┐       │
│  ATTENTION  │  │   RESULT    │       │
└──────┬──────┘  └──────┬──────┘       │
       │                 │ timeout      │
       └─────────────────┴──────────────┘
任意状態 ── failed ──▶ ERROR
ATTENTION ── high-risk approval ──▶ RISK

⸻

11. タイムアウトと復旧

Hookベースの設計では、終了イベントが必ず届くとは限らない。

プロセス強制終了、ターミナル終了、Hook不具合、ツール更新によって状態が残留する可能性がある。

そのため、以下を必須とする。

11.1 State TTL

状態	標準TTL
WORKING	30分
ATTENTION	4時間
RISK	30分
RESULT	8秒
ERROR	60秒
UNKNOWN	30秒

TTL経過後は、IDLEではなく一度UNKNOWNへ遷移させる。

11.2 Heartbeat

対応可能なProviderでは、処理中に一定間隔でheartbeatを送信する。

MVPではClaude Code Hookだけから完全なheartbeatを得ることは難しいため、プロセス存在確認を補助的に利用する。

11.3 Shell Prompt Reset

ユーザーのシェルプロンプトが再表示された際に、一定条件で状態を復旧するShell Integrationをオプション提供する。

11.4 終了シグナル

デーモンは以下を処理する。

* SIGTERM
* SIGINT
* ターミナル消失
* tmuxペイン消失
* Unix Socket切断

⸻

12. Activity Model

状態だけでなく、短い活動内容を表示できる。

WORKING · Running tests
WORKING · Editing files
WORKING · Searching code
ATTENTION · Approval required
RISK · Destructive command
RESULT · Task completed

12.1 Activity Category

thinking
reading
searching
editing
shell
testing
network
version_control
deployment
unknown

12.2 Activity Label制約

* 40文字以内
* 秘密情報を含めない
* コマンド全文を表示しない
* ファイルパスは標準でbasenameのみ
* Providerが渡した未検証文字列をそのまま制御シーケンスへ渡さない
* 制御文字を除去する

12.3 Activity表示の原則

Activityは補助情報であり、状態色の種類を増やす理由にはしない。

例えば「テスト実行」と「ファイル編集」はどちらもWORKINGであり、表示テキストのみ異なる。

⸻

13. Risk Classification

13.1 MVPでの扱い

MVPのRISKは、AIツール側が承認を要求したイベントのうち、明示的に高リスクと判断できる場合だけ使用する。

Agent Presence自身は、危険操作をブロックしない。

13.2 リスク分類候補

low
medium
high
critical
unknown

13.3 高リスク候補

* 本番環境へのデプロイ
* git push --force
* git reset --hard
* 大量ファイル削除
* terraform apply
* kubectl delete
* データベース破壊操作
* パッケージ公開
* 外部へのデータ送信

13.4 批判的判断

コマンド文字列の正規表現だけでRISKを決めるのは信頼性が低い。

例えばecho "kubectl delete"は危険ではない一方、危険な処理がラッパースクリプト内に隠れている場合は検知できない。

したがってMVPでは、リスク分類を安全機構として宣伝しない。

あくまで注意喚起として扱い、将来的なポリシーエンジンは別コンポーネントにする。

⸻

14. Renderer Capability Model

ターミナルごとの能力差を、共通インターフェースで吸収する。

interface RendererCapabilities {
  background: boolean;
  tabTitle: boolean;
  tabColor: boolean;
  badge: boolean;
  cursorColor: boolean;
  paneBorder: boolean;
  notification: boolean;
  flash: boolean;
  resetReliable: boolean;
}

14.1 Rendererインターフェース

interface Renderer {
  detect(context: TerminalContext): Promise<RendererCapabilities>;
  render(state: PresenceView): Promise<void>;
  reset(target: RenderTarget): Promise<void>;
  healthCheck(): Promise<HealthStatus>;
}

14.2 MVP Renderer

iTerm2 Renderer

候補機能：

* セッションプロファイル切り替え
* タブまたはウィンドウタイトル
* バッジ
* 背景色
* 通知

tmux Renderer

候補機能：

* ペイン枠色
* ペインタイトル
* ウィンドウステータス
* ステータスバー

macOS Notification Renderer

通知対象：

* ATTENTION
* RISK
* RESULT
* ERROR

WORKINGでは通知しない。

OSC Renderer

OSCなどの制御シーケンスを利用する汎用Rendererを将来的に提供する。

WezTermは動的なカラーパレット変更をエスケープシーケンスから実行できるため、将来の対象として実現性がある。(WezTerm)

⸻

15. 標準テーマ

15.1 標準表現

状態	色	アイコン	通知
IDLE	デフォルト	○	なし
WORKING	青	●	なし
ATTENTION	オレンジ	!	あり
RISK	赤	⚠または!!	強い通知
RESULT	緑	✓	任意
ERROR	マゼンタまたは暗赤	×	あり
UNKNOWN	グレー	?	原則なし

絵文字の表示幅問題を避けるため、ASCIIアイコンセットも提供する。

15.2 色覚対応テーマ

以下を同梱する。

* Default
* Color Vision Safe
* Low Distraction
* High Contrast
* Monochrome Symbols

15.3 背景色モード

rendering:
  background:
    enabled: false
    intensity: subtle

背景色は標準では無効とする。

有効化した場合も、完全な単色背景ではなく、暗い色調のわずかな変更を推奨する。

⸻

16. 設定ファイル

version: 1
daemon:
  enabled: true
  log_level: warn
  event_retention: 24h
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
renderers:
  tmux:
    enabled: auto
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
providers:
  claude:
    enabled: true
  opencode:
    enabled: false
  codex:
    enabled: false
    experimental: true

⸻

17. Claude Code統合

17.1 利用候補イベント

Claude CodeのHookイベントを、以下のようにマッピングする。

Claude Codeイベント	Normalized Event	Presence State
SessionStart	session.started	IDLE
UserPromptSubmit	agent.started	WORKING
PreToolUse	tool.started	WORKING
PostToolUse	tool.completed	WORKING
PostToolUseFailure	tool.failed	ERRORまたはWORKING
Notification	内容により分類	ATTENTION
Stop	agent.completed	RESULT
SessionEnd	session.stopped	IDLE

実際のマッピングはHookイベント名だけでなく、入力JSONの内容とClaude Codeのバージョンに基づいて決定する。Claude CodeのHookリファレンスはイベントスキーマや設定方法を公開しており、設定ファイル変更も多くの場合は実行中セッションへ再読込される。(Claude Platform Docs)

17.2 Hookコマンド

presence ingest --provider claude

JSONは標準入力から受け取る。

Claude Code
   │ JSON stdin
   ▼
presence ingest --provider claude
   │ normalized event
   ▼
Unix Domain Socket
   │
   ▼
presence daemon

17.3 インストール方式

presence install claude --scope user
presence install claude --scope project
presence install claude --scope local

既存設定を破壊せず、設定ファイルを構文解析して必要なHookだけ追加する。

アンインストール時にはAgent Presenceが追加したエントリだけを削除する。

⸻

18. セッション識別

18.1 識別キー

優先順位：

1. Providerが提供するsession ID
2. tmux pane ID
3. TTY
4. terminal session ID
5. workspaceとプロセスIDの組み合わせ

18.2 tmux対応

tmux内ではターミナル全体ではなく、対象ペインだけを更新する。

TMUX_PANE=%12

をセッションターゲットとして利用する。

18.3 サブエージェント

サブエージェントが生成される場合、以下のモデルを使用する。

Parent Session
 ├─ Subagent A: WORKING
 ├─ Subagent B: ATTENTION
 └─ Subagent C: RESULT

親セッションの表示は子の最優先状態を集約する。

ただしProvider側がメインエージェントとサブエージェントを識別できない場合は、無理に推測せず同一セッションとして扱う。

CodexではHookイベントからメインエージェントとサブエージェントを識別しにくいという報告があり、共通仕様側で親子関係を必須にすると統合できないProviderが生じる。(GitHub)

⸻

19. 通知抑制

複数のHookイベントにより、同じ状態が短時間に連続する可能性がある。

19.1 デバウンス

標準値：

同一状態の再描画: 250ms
同一通知の再送: 10秒
RESULT通知の再送: 30秒

19.2 フォーカス判定

対象ターミナルが現在フォーカスされている場合、ATTENTIONとRESULTのmacOS通知を抑制できる。

RISKはフォーカス中でも表示する。

19.3 Quiet Hours

notifications:
  quiet_hours:
    start: "22:00"
    end: "07:00"
    allow:
      - risk

⸻

20. セキュリティ

20.1 脅威

* Hook入力に悪意ある文字列が含まれる
* ターミナル制御シーケンスが注入される
* 他プロセスが偽イベントを送る
* Socket権限が不適切
* ログへ機密情報が残る
* 設定インストール時に既存Hookを破壊する
* Agent Presenceが高権限コマンドを実行する

20.2 対策

* 受信JSONをスキーマ検証する
* 表示文字列から制御文字を削除する
* Socket権限をユーザーのみへ制限する
* イベントサイズ上限を設定する
* Hook入力をシェル評価しない
* コマンド文字列を再実行しない
* デフォルトで本文をログ保存しない
* 設定変更前にバックアップを作成する
* install時に差分を表示できる
* Rendererは許可された制御シーケンスだけ生成する

20.3 イベントサイズ

標準上限：

64 KiB / event

超過イベントは拒否し、メタデータのみ記録する。

⸻

21. 信頼性要件

21.1 Fail-open

Agent Presenceが停止していても、AIエージェントの処理は続行する。

21.2 Idempotency

同一event_idを複数回受信しても、状態遷移と通知を重複させない。

21.3 Atomic Configuration

設定更新は一時ファイルへ書き込み、検証後にatomic renameする。

21.4 Crash Recovery

デーモン再起動後は、残留状態をそのまま再表示せず、セッション存在を確認する。

確認できない状態はUNKNOWNを経てIDLEに戻す。

21.5 Reset Guarantee

次のコマンドで、すべての表示を強制的に既定状態へ戻せる。

presence reset --all

⸻

22. パフォーマンス要件

項目	目標
Hook CLI実行時間	50ms以下
イベント反映時間	150ms以下
デーモン常駐メモリ	30MB以下
アイドルCPU使用率	0.1%未満
1秒当たりイベント処理	100イベント以上
起動時間	300ms以下

これらは初期目標であり、実装言語とRenderer方式に応じて調整する。

⸻

23. 技術スタック候補

23.1 推奨

* 実装言語：Rust
* 非同期ランタイム：Tokio
* CLI：clap
* 設定：serde、serde_yaml
* ローカル通信：Unix Domain Socket
* ログ：tracing
* macOS通知：osascript依存を避け、可能ならネイティブAPIまたは安定した通知ライブラリ
* 配布：Homebrew Tap、GitHub Releases

23.2 Rustを選ぶ理由

* 単一バイナリ配布
* 常駐プロセスの低メモリ化
* Unix Socketとの親和性
* シェルHookから高速起動
* クロスプラットフォーム展開の余地
* 入力検証を型で表現しやすい

23.3 代替案

最初の技術検証はTypeScriptまたはPythonでも構わない。

ただし製品版までPythonで進める場合、CLI起動時間、依存関係、配布、常駐プロセス管理が課題になりやすい。

⸻

24. ディレクトリ構成案

agent-presence/
├── crates/
│   ├── presence-cli/
│   ├── presence-daemon/
│   ├── presence-core/
│   ├── presence-provider-claude/
│   ├── presence-renderer-iterm2/
│   ├── presence-renderer-tmux/
│   └── presence-renderer-notification/
├── schemas/
│   └── event-v1.schema.json
├── themes/
│   ├── default.yaml
│   ├── color-safe.yaml
│   └── low-distraction.yaml
├── integrations/
│   └── claude/
├── docs/
└── tests/

⸻

25. テスト仕様

25.1 Unit Test

* Providerイベント変換
* 状態遷移
* 優先順位
* TTL処理
* 重複イベント排除
* 制御文字除去
* リスク分類
* テーマ解決

25.2 Integration Test

* Hook JSONからtmux表示まで
* デーモン停止時のfail-open
* Socket権限
* 設定インストールと復元
* 複数セッションの分離
* イベント順序逆転
* Hook欠落時のタイムアウト
* ターミナル終了時のリセット

25.3 End-to-End Test

Claude Code開始
  ↓
WORKING表示
  ↓
ツール実行
  ↓
WORKING + Activity更新
  ↓
承認待ち
  ↓
ATTENTION + 通知
  ↓
ユーザー承認
  ↓
WORKING
  ↓
完了
  ↓
RESULT
  ↓ 8秒
IDLE

25.4 Fault Injection

* デーモンを強制終了
* Hook入力を途中で切断
* 不正JSONを送信
* 1000イベントを連続送信
* tmuxを途中で終了
* iTerm2を途中で終了
* 古いProviderスキーマを送信
* タイムスタンプが逆転したイベントを送信

⸻

26. MVP受け入れ条件

以下をすべて満たした場合、MVP完成とする。

1. presence install claudeで既存設定を壊さずHookを追加できる
2. Claude Code開始後に対象ペインがWORKINGになる
3. ユーザー入力または承認待ちをATTENTIONとして表示できる
4. 完了時にRESULTを表示し、一定時間後に元へ戻る
5. tmuxの別ペインへ状態が漏れない
6. Agent Presence停止中でもClaude Codeが正常動作する
7. 強制終了後も表示状態を手動リセットできる
8. Hook入力内容が標準では永続保存されない
9. presence doctorが主要な設定不良を検出する
10. 30分の連続利用で明らかな状態残留が発生しない

⸻

27. 開発フェーズ

Phase 0：技術スパイク

目的：

* Claude Code Hookからイベントを取得
* tmuxペインを特定
* ペイン枠色を変更
* 完了後に復旧

成果物：

presence event working
presence event attention
presence event result

期間ではなく、技術的成立条件を満たした時点で終了する。

Phase 1：MVP

* Claude Code Provider
* State Engine
* tmux Renderer
* iTerm2 Renderer
* macOS通知
* install、doctor、reset
* 標準テーマ
* Homebrew配布

Phase 2：複数エージェント

* OpenCode Provider
* 複数セッション一覧
* サブエージェント集約
* WezTermまたはGhostty対応
* テーマ共有

Phase 3：拡張基盤

* Provider SDK
* Renderer SDK
* Generic WebhookまたはSocket API
* VS Code、Cursor連携
* メニューバーアプリ
* チーム向け設定配布

⸻

28. プロダクト上の批判的レビュー

28.1 「色が変わる」だけでは継続利用されない

最初は目新しくても、単一セッションだけを使うユーザーには価値が小さい。

本製品が本当に価値を持つのは、複数のAIエージェントを並列実行する場合である。

したがって、訴求軸は「ターミナルを美しくする」ではなく、次のようにする。

複数AIエージェントの中から、自分の対応を待っているセッションを即座に見つける。

28.2 背景色変更を中核にすべきではない

背景色はデモ映えするが、実用上の副作用が多い。

中核機能は状態管理とイベント正規化であり、背景色はRendererの一つにとどめる。

28.3 汎用CLIプラットフォーム化は早すぎる

Git、Docker、Terraform、CIなどへ広げると、対象イベント、状態意味、競合製品が急増する。

初期段階では「AIコーディングエージェントの注意状態」に集中し、利用者が増えてから汎用イベント対応を検討する。

28.4 危険操作検知を売りにすると責任が大きい

誤検知と見逃しの両方が避けられない。

安全機構として販売するには、単純な表示ツールよりはるかに高い品質保証が必要になる。

MVPでは「危険操作を防ぐ」と表現せず、「承認要求を目立たせる」と表現する。

28.5 Codex対応をMVPに含めるべきではない

2026年時点のCodex Hook周辺には、利用可能なイベント不足、バージョン間の変更、対話セッションでの不具合などの報告がある。正式サポートにすると、製品側がCodexの実験的仕様に引きずられる可能性がある。(GitHub)

Codex対応は以下のいずれかとする。

* Experimental
* Completion通知だけ対応
* 公式イベント仕様の安定後に正式対応

28.6 デーモンはMVPに本当に必要か

Claude Codeと単一tmuxペインだけなら、Hookから直接色を変える実装でも成立する。

しかし、以下を実現するにはデーモンが必要になる。

* 状態競合の解決
* TTL
* 通知抑制
* 複数セッション管理
* Provider横断
* クラッシュ復旧
* サブエージェント集約

したがって技術スパイクではデーモンなし、MVPではデーモンありとする。

⸻

29. 最終的なMVP定義

MVPを次の一文で定義する。

Claude Codeを複数のtmuxペインで実行している開発者が、作業中・入力待ち・完了・異常を、ペインを開かずに識別できるローカルCLIツール。

MVP構成：

Claude Code Hooks
        ↓
presence ingest
        ↓
Local Daemon
        ↓
State Engine
        ↓
tmux pane border + title
        ↓
iTerm2 badge + macOS notification

標準状態：

IDLE
WORKING
ATTENTION
RESULT
ERROR

RISKは内部仕様として準備するが、初期UIではATTENTIONの強調表示として扱う。

⸻

30. 成功指標

初期段階ではダウンロード数より、継続利用を重視する。

プロダクト指標

* インストール成功率：90%以上
* 7日後継続利用率：30%以上
* 1日当たりのATTENTION検出数
* 状態残留エラー率：1%未満
* 手動reset実行率
* 通知無効化率
* アンインストール理由

検証すべき仮説

1. ユーザーは背景色よりペイン枠を好むか
2. 完了通知より入力待ち通知に価値を感じるか
3. 単一エージェント利用者にも価値があるか
4. 状態表示は5種類で十分か
5. Agent Presenceのためにtmuxを導入する利用者がいるか
6. iTerm2単体ユーザーにも同等の価値を提供できるか

⸻

31. プロダクト名候補

「Terminal Presence Engine」は技術基盤としては適切だが、利用者に価値が伝わりにくい。

候補：

* Agent Presence
* Agent Beacon
* Agent Signal
* Terminal Beacon
* Agent Glow
* Terminal Pulse
* Agent Watch

推奨仮称：

Agent Presence

理由：

* 背景色だけに機能を限定しない
* 複数エージェントの状態表示を表現できる
* 将来的なメニューバー、IDE、Web UIにも展開できる
* 「監視」よりも柔らかく、常駐状態の可視化を示せる

⸻

32. 最終判断

本製品で最も重要な資産はテーマでもターミナル制御コードでもない。

中核資産は以下の3つである。

1. 異なるAIツールのイベントを共通形式へ変換するProvider層
2. 不完全で順序が乱れるイベントから、安定した状態を生成するState Engine
3. ターミナルごとの機能差を吸収するRenderer層

したがって、開発の優先順位は次の順とする。

イベント取得の信頼性
    >
セッション識別
    >
状態遷移と復旧
    >
tmux表示
    >
通知
    >
テーマ
    >
背景色演出

デモでは背景色が最も目立つが、プロダクト品質を決めるのは状態が正しく戻ること、別セッションへ表示が漏れないこと、AIツール本体を妨げないことである。
