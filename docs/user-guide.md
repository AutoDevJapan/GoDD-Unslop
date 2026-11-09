# GoDD Unslop ユーザーガイド

[PowerPoint版](user-guide.pptx)は従来の分析・比較・修正例の保存を説明する20枚です。AI改稿とSkill導入、追加されたMCPツールは未反映のため、この文章版を参照してください。
操作例で使うファイルは[サンプルフォルダー](guide-examples/)から入手できます。

GoDD Unslopは、文章を見直す候補と推敲の手順を返すツールです。
Skillで下書きから書き方を整える方法と、Codex CLIで既存の文章を改稿する方法があります。分析機能だけを使い、自分で修正を選ぶこともできます。

CLI・Rustライブラリ・MCPサーバーとして利用できます。分析・比較はモデルを呼ばず、AI改稿はChatGPTサブスクにログインしたCodex CLIを使います。APIキーは使いません。
敬語かため口か、作者が人間かAIかを採点するものではありません。

## インストールと更新

Rust 1.95以降とGitを用意し、Rustのプログラムをビルドできる環境で実行してください。

```sh
git clone https://github.com/AutoDevJapan/GoDD-Unslop.git
cd GoDD-Unslop
cargo install --path . --locked
godd-unslop --help
```

コマンドが見つからない場合は、Cargoのインストール先の`bin`フォルダーをPATHへ追加します。
標準の場所はWindowsなら`%USERPROFILE%\.cargo\bin`、macOS・Linuxなら`$HOME/.cargo/bin`で、`CARGO_HOME`を変更している場合はその配下です。

更新するときは、このリポジトリで`git pull --ff-only`を実行し、もう一度`cargo install --path . --locked`を実行します。

この版の`rewrite`と`install-skill`を使うには、上記のインストールで実行ファイルを更新してください。古いバイナリには追加コマンドがありません。

## AIに文章を書いてもらうためのSkillを入れる

最初の下書きから書き方を整えたい場合は、使っているAIアプリへSkillを導入します。
アプリが読み込むSkillのルートフォルダーを確認し、次のように指定してください。アプリの設定ファイルや既存Skillは変更しません。

Codexでホーム配下のSkillsを使う場合の例です。別のアプリや配置へ合わせる場合は保存先を置き換えます。

```powershell
# Windows / PowerShell
godd-unslop install-skill (Join-Path $HOME '.codex/skills')
```

```sh
# macOS・Linuxでホーム配下を使う場合
godd-unslop install-skill "$HOME/.codex/skills"
```

共有のSkill置き場を使っている環境では、ホーム配下へ新設せず、既存の共有先を指定してください。
保存先の下に`godd-unslop/SKILL.md`が作られます。同じ内容があれば成功し、異なる既存ファイルがあれば上書きせずエラーを返します。
更新時に内容が異なる場合は、自分の変更を別途保存してから内容を比較してください。再インストールだけで既存Skillが更新されるわけではありません。
アプリのSkill一覧で読み込まれたことを確認し、必要ならそのアプリの手順で再読み込みするか新しい会話を開きます。

「godd-unslopのSkillを使って、利用者への変更案内を書いて」と依頼し、読者・事実・伝える結果を添えます。
このSkillは、利用中のAI自身に文のつながりや段落の方針を渡します。別のモデル呼び出しや`rewrite`の再帰実行は必要ありません。
CLIを使えない環境でも[Skill本文](distribution/godd-unslop-SKILL.md)を読ませて利用できます。導入によってモデルの重みが変わったり、自然な日本語が常に保証されたりするものではありません。

## Codex CLIで文章を書き直す

既存の文章の改稿をJSONで受け取りたい場合に使います。Codex CLIを導入し、ChatGPTサブスクのアカウントでログインしておきます。
APIキーのログインでは実行しません。利用枠が足りない場合は、その契約の利用状況を確認してください。

起動側で`GODD_UNSLOP_CODEX`を設定すると、呼び出す実行ファイルを指定できます。指定できるのは、運用者が信頼して選んだCodex実行ファイルです。入力JSONから実行先を選ぶことはできません。
未指定ならWindowsでは`codex.exe`、macOS・Linuxでは`codex`をPATHから探します。
Windowsのnpm経由の`codex.cmd`・`codex.ps1`は直接起動できないため、実際のネイティブ実行ファイルの絶対パスを指定してください。

```powershell
# パスは実際に導入した実行ファイルへ置き換えます。
$env:GODD_UNSLOP_CODEX = 'C:/path/to/codex.exe'
& $env:GODD_UNSLOP_CODEX login status
```

```sh
# PATHにあるcodexを使う場合
codex login status
# 別の実行ファイルなら、その絶対パスを指定します。
# export GODD_UNSLOP_CODEX=/path/to/codex
```

`login status`でChatGPTを使ったログインであることを確認します。未ログインなら、指定した実行ファイルで`login`を実行し、表示された認証手順を進めてください。

UTF-8の`rewrite-request.json`に次の内容を保存します。必須項目は`text`です。

```json
{"text":"確認を実施しました。結果は問題なしです。公開作業への移行が可能な状態です。"}
```

```sh
godd-unslop rewrite rewrite-request.json
```

`rewritten_text`が改稿本文です。`original`と`comparison`、`meaning_warnings`、`layout_warnings`も返るため、条件・主体・否定・数量が変わっていないか確認してから採用します。
改行と空行は別に扱い、同じ話題の中の意味のまとまりは改行、話題が変わる距離は空行として考えます。原文にある意図した段落は必要なく減らさず、句点があるだけで機械的に区切ることも、すべてを一文や一行にまとめることもしません。
`layout_warnings`は、原文の空行で区切られた段落が減った場合や、複数行だった原文が1行になった場合の確認用です。誤りを断定せず、意味のまとまりが保たれているか読んで判断します。
改稿はJSONとして返り、元ファイルを上書きしません。必ず異なる文章になるとは限らず、意味の一致や日本語の品質を保証する結果でもありません。
`rewrite`を選ぶと原文と補助設定をCodex経由でChatGPTへ送ります。分析だけの場合とは異なります。

任意の`profile`には保存した文体設定を、`preserve`には原文中で保持したい文字列の配列を渡せます。保持語は最大64件、各256バイトで、原文中に存在する必要があります。
`profile`は意味解析の結果ではなく修正例です。会話から自動収集したり、この呼び出しの結果を自動学習したりはしません。
`rewrite`の本文上限はUTF-8で128KiB、CLIのJSON全体は2MiBです。Codexの認証確認は15秒、改稿は120秒の制限があり、認証・利用枠・実行ファイル・保持表記の検査で失敗した場合はエラーを返します。

## 最初の文章を確認する

文章をUTF-8の`article.txt`として保存して実行します。

```sh
godd-unslop analyze article.txt
```

ファイルを省略するか`-`を指定すると標準入力から読み込みます。
macOS・Linuxなどのシェルでは、次のように短い文章を渡せます。

```sh
printf '%s' '既定の保存先です。' | godd-unslop analyze -
```

PowerShell 7以降では、次のように渡せます。

```powershell
'既定の保存先です。' | godd-unslop analyze -
```

結果はJSONで返り、`--json`を省略しても形式は変わりません。

## 結果を読む

| 項目 | 読み方 |
| --- | --- |
| `findings` | 見直す候補の一覧。問題の確定や自動修正の命令ではありません。 |
| `rule` | 候補を返したルールの識別子。 |
| `explanation` | 見直す候補になった理由。 |
| `review_action` | 文脈と照合するときに確認する内容。 |
| `start`・`end` | 原文上の範囲。`end`が指す位置の文字は含みません。 |
| `metrics` | 文の長さや漢字率などの数値。 |
| `findings_limited` | 上限のために返しきれない候補があるか。 |

位置の`byte`と`character`は0から、`line`と`column`は1から数えます。
先頭BOMは本文の検査から外しますが、返す位置にはBOMも含みます。

たとえば「既定」が指摘されても、読み手に意味が伝わる文脈なら残して構いません。
指摘がないことも、文章の正しさや意味の保持を保証するものではありません。

## 推敲して原文と比べる

`plan`は原文と分析結果に、内容を保つ推敲手順を添えて返します。

```sh
godd-unslop plan article.txt
```

用途を添える場合は、次の内容をUTF-8の`plan-request.json`に保存して渡します。

```json
{"action":"plan","text":"保存先を変更しました。","context":"利用者への変更案内"}
```

```sh
godd-unslop dispatch plan-request.json
```

自分で修正した文章を`revised.txt`に保存し、原文と比較します。

```sh
godd-unslop compare article.txt revised.txt
```

`changed_region`は変更箇所をすべて含む一つの範囲で、個々の変更を並べた差分ではありません。
`numeric_fragments_changed`は対応済みの数量表現、`listed_marker_counts_changed`は列挙した語の件数の変化を示します。

数や語の件数が同じでも、主体・条件・否定・確かさが変わることがあります。
`semantic_equality`は`not_established`として返るため、最後に原文と改稿の全文を照合してください。

## 改行と空行を修正例から学ぶ

意味のつながる言葉や文節、節や連を保ち、その境界とまとまり同士の距離を改行・空行で表します。
句点や文字数だけで分断せず、実際にどう直したいかを修正前後の例で示してください。

`style-learn`は修正例から`profile`を作り、`style-review`はその設定で文章を照合します。
以下はPowerShell 7以降を使い、リポジトリとは別の空の作業フォルダーで実行する手順です。

### 修正例を渡して設定を保存する

次の内容をUTF-8の`learn-request.json`に保存します。
JSON文字列の`\n`は改行、`\n\n`は改行を二つ続けた空行を表します。

```json
{
  "examples": [
    {
      "before": "記録を保存しました。次に確認します。",
      "after": "記録を保存しました。\n次に確認します。"
    },
    {
      "before": "確認は終わりました。\n次の案件を説明します。",
      "after": "確認は終わりました。\n\n次の案件を説明します。"
    }
  ]
}
```

```powershell
$learnedJson = godd-unslop style-learn learn-request.json | Out-String
if ($LASTEXITCODE -ne 0) { throw '修正例を学習できませんでした。' }
$learned = ConvertFrom-Json $learnedJson
[IO.File]::WriteAllText(
    (Join-Path $PWD 'profile.json'),
    ($learned.profile | ConvertTo-Json -Depth 20)
)
```

保存するのは結果全体ではなく、その中の`profile`です。
`ignored_examples`が0でない場合は、学習条件に合わず取り込まれなかった例があります。

### 保存した設定で文章を照合する

```powershell
$profile = Get-Content profile.json -Raw -Encoding utf8 | ConvertFrom-Json
$request = @{
    text = '記録を保存しました。次に確認します。'
    profile = $profile
} | ConvertTo-Json -Depth 20
[IO.File]::WriteAllText((Join-Path $PWD 'review-request.json'), $request)
godd-unslop style-review review-request.json
```

この例では、二つの文の間に改行を置く候補が返ります。
`choices`の`spacing`が`same_line`なら同じ行、`line_break`なら改行、`paragraph_distance`なら空行による距離を表し、`blank_lines`は空行の数です。

相反する修正例は`layout_conflict`などの競合として複数の候補を返し、登録順だけでどちらかを正解にはしません。

### 修正例を追加する

既存の設定を`profile`に、新しい修正例を`examples`に入れて学習します。

```powershell
$profile = Get-Content profile.json -Raw -Encoding utf8 | ConvertFrom-Json
$request = @{
    profile = $profile
    examples = @(@{
        before = "結果を確認し、`n記録しました。"
        after = '結果を確認し、記録しました。'
    })
} | ConvertTo-Json -Depth 20
[IO.File]::WriteAllText((Join-Path $PWD 'append-request.json'), $request)
$learnedJson = godd-unslop style-learn append-request.json | Out-String
if ($LASTEXITCODE -ne 0) { throw '修正例を追加できませんでした。' }
$learned = ConvertFrom-Json $learnedJson
[IO.File]::WriteAllText(
    (Join-Path $PWD 'profile.json'),
    ($learned.profile | ConvertTo-Json -Depth 20)
)
```

追加後の設定を保存しておくと、次回の起動時にも使えます。
失敗した結果で保存済みの設定を上書きしないよう、終了コードを確認してから保存してください。

### 学習できる範囲

現在は、修正前後の文字・数字の並びを保ち、配置や句読点を変えた例を扱います。
単語の言い換えや敬語への変更、Markdownや引用を含む例は取り込まれない場合があります。

改行の照合は前後それぞれ最大4文字の局所的な文字列を手がかりにし、同じ意味の言い換え全般へ一般化するものではありません。
文節・構文・談話の意味解析、会話の自動収集、起動をまたぐ自動保存、本文の自動修正は行いません。

個人の修正例や`profile`には元の語句が含まれるため、公開リポジトリへコミットせず、公開してよい文章だけを共有してください。

## 相手・場面・背景を指定する

同じ用件を相手に合わせて伝えるには、`rewrite`の入力へ`context`を追加します。
たとえば、親しい同僚へのチャットなら次のように指定できます。

```json
{
  "text": "資料を火曜日17時までに送ってください。",
  "context": {
    "purpose": "資料の送付を依頼する",
    "recipient": {"relation": "peer", "closeness": "close"},
    "tone": {"formality": 2, "warmth": 3, "directness": 2, "emoji": "light"},
    "channel": "chat"
  }
}
```

`formality`、`warmth`、`directness`は0〜4で調整します。職位だけで一律の口調にはせず、相手との関係や用途を合わせて使います。
公式文書は`channel`を`official`にすると絵文字を使わない扱いになります。

`context`には`purpose`（目的）、`audience`（読み手）、`document_type`（文書の種類）、`situation`（背景）、`stance`（立場）、`company_style`（会社の文体）、`reference_examples`（参考例）も渡せます。
会社独自の文体が必須なら`company_style_required`を`true`にし、文体の説明か参考例を渡してください。

重要な不明点は`missing_facts`へ質問の配列で指定できます。必要な情報が不足している場合は、`status: needs_context`と`questions`を返し、AIによる改稿は実行しません。
Skillを使うAIは、まず提供資料や許可された参照先を調べ、それでも分からない点を質問します。CLI自体が他のファイルや連絡先を探すことはありません。

相手の設定を毎回書かずに使う方法は、[関係性DBのガイド](recipient-context.md)を参照してください。
保存した相手を指定する`rewrite-for`は改稿本文を返すだけで、メールやチャットの送信は行いません。

## 文章の特徴を調べる

```sh
godd-unslop features article.txt
godd-unslop features --catalog
```

文長は句点・疑問符・感嘆符や段落などのブロック境界で区切り、空白と区切り文字を除いて数えます。
段落内の改行で一文を短く数えることはなく、漢字率や平均文長も品質や作者を採点する点数ではありません。

## MCPクライアントから使う

MCPクライアントに、`godd-unslop serve`を標準入出力で起動するサーバーとして登録します。
`mcpServers`形式を使うクライアントなら、次の設定をそのクライアントの設定ファイルへ組み込めます。

```json
{
  "mcpServers": {
    "godd-unslop": {
      "command": "godd-unslop",
      "args": ["serve"]
    }
  }
}
```

設定ファイルの場所や形式はクライアントによって異なります。
GUIから起動するクライアントでPATHが引き継がれない場合は、`command`へ実行ファイルの絶対パスを指定してください。

| ツール | 主な入力 | 返す内容 |
| --- | --- | --- |
| `unslop_rewrite` | `text`、任意の`profile`・`preserve` | Codex経由の改稿と原文比較。 |
| `unslop_analyze` | `text` | 指摘と文章の特徴。 |
| `unslop_plan` | `text`、任意の`context` | 原文と用途を保つ推敲手順。 |
| `unslop_compare` | `original`、`rewrite` | 差分と数量・語の件数の比較。 |
| `unslop_features` | 任意の`text` | 機能一覧。文章を渡すと特徴の数値も返します。 |
| `unslop_style_learn` | `examples`、任意の`profile` | 更新した設定と無視した例の件数。 |
| `unslop_style_review` | `text`、`profile` | 配置・句読点の変更候補。 |

`unslop_rewrite`はMCPサーバー側のCodexログインと実行ファイルを使います。Skillだけの利用と異なり、別途AIを呼び出します。

MCPの引数には`action`を含めず、表にある項目だけを渡してください。
CLIの`dispatch`へJSONを渡す場合は、`action`に`rewrite`、`analyze`、`plan`、`compare`、`features`、`style_learn`、`style_review`のいずれかを指定します。

`serve`はHTTPサーバーではなく、標準入力でMCPの要求を待つプログラムです。
ターミナルから起動して何も表示されなくても、クライアントが接続するまでは通常の待機状態です。

## 上限と終了コード

| 対象 | 上限・条件 |
| --- | --- |
| 文章1件 | UTF-8で128KiB。 |
| CLIのJSON入力全体 | 2MiB。文章自体の上限は別に適用します。 |
| `analyze`・`style-review`の指摘 | それぞれ128件まで。超過時は`findings_limited`を確認します。`compare`内の原文・改稿の分析にも、それぞれの上限があります。 |
| 修正前・修正後の文章 | それぞれUTF-8で4096バイトまで。空の文章や同一の文章は指定できません。 |
| 1回の学習入力・保存設定 | それぞれ最大64例。既存例と追加例を合わせた保存設定にも適用します。 |
| 文体設定 | JSONへ変換した状態で64KiBまで。現在の`version`は1です。 |

| 終了コード | 意味 |
| --- | --- |
| `0` | コマンドの処理が成功しました。通常は指摘が含まれる場合も0です。 |
| `1` | `--strict`を指定し、`analyze`・`plan`・`style-review`で候補があったか、`compare`で文章に差がありました。 |
| `2` | 入力やコマンドを確認してください。理由を標準エラー出力へ返します。 |

```sh
godd-unslop analyze article.txt --strict
godd-unslop compare article.txt revised.txt --strict
```

`compare --strict`は文章に差があるだけでも1を返し、悪い改稿かどうかを判定するものではありません。
`style-learn`で無視された例があっても、それだけで1にはなりません。

## 困ったときは

| 状況 | 確認すること |
| --- | --- |
| UTF-8のエラーが出る | UTF-8で保存し直してください。PowerShellの手順は7以降を前提にしています。 |
| 未対応の項目と言われる | JSONのキーと型、`action`とMCP引数の違いを確認してください。 |
| 設定を渡せない | 学習結果全体ではなく、中の`profile`を渡しているか確認してください。 |
| 指摘が出ない | 保護範囲か、学習した文字列の条件に合わない可能性があります。 |
| 指摘が多すぎる | `explanation`と`review_action`を読み、文脈で意味が伝わる表現は残してください。 |
| 設定がいっぱいになる | 64例・64KiBの上限を確認し、変更前の設定を別途保存してから不要な例を整理してください。 |

## ライセンスと関連資料

現在の公開版はGPLv3以降で提供しています。
自己保有コードの個別許諾と過去のMIT許諾は、[README](../README.md#ライセンス)、[LICENSE](../LICENSE)、[NOTICE](../NOTICE)を確認してください。

データの扱いは[収集方針](data-policy.md)、過去の測定と現在の機能の違いは[評価の説明](evaluation.md)にまとめています。
