# GoDD Unslop

MakhmetoがAIとの作業で指摘してきた読みにくさを、Rustの検出ルールと推敲手順にまとめるツールです。結果が背景に埋もれる説明、内部の番号だけの報告、名詞が続いて動作を読めない文、意味が曖昧な用語、状態と本文の食い違いを見直します。

ライブラリ、CLI、MCPサーバーを提供します。APIキーやモデルの呼び出しは必要ありません。指摘は文脈を読んで判断する候補です。自然な文や意味を担う対比は残します。

## 使い方

```sh
cargo install --path . --locked
printf '既定の保存先です。' | godd-unslop analyze --json
printf '保存の確認はこれからです。' | godd-unslop plan --json
godd-unslop compare original.txt revised.txt --json
godd-unslop features article.md --json
godd-unslop features --catalog
godd-unslop serve
```

`analyze`は位置と見直し理由を返します。位置は0から始まるUTF-8バイト位置とUnicode文字位置、1から始まる行・列です。UTF-8の先頭BOMは本文の検査から外しますが、位置はBOMを含む原文に対する値です。`plan`は元文章と用途を保つ手順を返し、本文を書き直しません。用途などを添える場合は、`dispatch`へ `{"action":"plan","text":"...","context":"..."}` を標準入力で渡します。

`compare`は変更された外側の範囲、数字・漢数字と列挙した単位を出現順で比べた結果、意味を担う語の件数、前後の指摘を返します。複数の変更を一つの範囲にまとめます。数が同じでも条件や主体が変わるため、意味が同じだと自動で認定しません。

`features`は入力文章の特徴を返します。`--catalog`なら対応機能の一覧だけを返します。特徴は文の長さ、読点数、漢字率、見出し・リスト・強調の数です。定義はCommonMarkから取り出した文字で、空白と区切り文字の扱いを区別します。本文文字数は空白を除き句読点を含みます。文長は句点・疑問符・感嘆符・段落などのブロック境界で区切り（段落内の改行では区切りません）、空白と区切り文字を除きます。日本語として完全な文かどうかは判断しません。

入力は1本文128KiBまで。CLIは終了コード0で成功、2で入力エラー。`--strict`は指摘や改稿差分があれば1を返します。引用、Markdownのコード・リンク・見出し・設定ブロックは検出から外します。裸のURLはASCIIの範囲を保護します。日本語を含むURLは `<https://example.test/日本語>` やMarkdownリンクで範囲を明示してください。文章をそのまま保ち、変更候補の確認だけを行います。

## MCPとライブラリ

stdio MCPには `unslop_analyze`、`unslop_compare`、`unslop_plan`、`unslop_features` があります。ツール引数へ文章を明示して呼びます。MCPからファイルパスを指定したり、サイトの文章を自動取得したりはしません。

MCPを使わないRustの組み込みでは、`default-features = false`を指定できます。`analyze`、`compare`、`plan`、`features`とJSON用の`dispatch`を公開します。ツールの定義は`rpc::tools()`で取得できます。

## 知見と特徴データ

ルールはオーナーの明示した改善要求から新しく実装しました。要件は [owner-requirements.json](docs/owner-requirements.json) にまとめています。既存ツールのコード・指示文・辞書・検証例を転記していません。

新たに公開ページ4件を取得し、オーナーの指摘4件、新規AI例文4件と合わせて特徴を集計しました。元の本文は含めず、数値と取得条件だけを [data](data) に保存しています。制作方法が不明な公開文章は`unknown`です。詳細は [収集方針](docs/data-policy.md) と [評価](docs/evaluation.md) を参照してください。

この小規模なデータは抽出機能の確認用です。人間・AIの分類器や、校正済みの文章品質スコアではありません。ジャンルと長さが異なる群の平均から、作者や改善効果を判断しません。文長や漢字列の目安も経験に基づく候補で、科学的な判定閾値として扱いません。

## 検証

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo test --no-default-features --locked
```

自然な文、引用、コード、数量や条件の変更、UTF-8位置、CLI終了コード、MCPの実通信を検証します。CIはUbuntuとWindowsで実行します。

Rust 1.95以降を使います。MIT License。非公開で開発中です。公開への切り替えはオーナーが決めます。



## 修正例から改行と空行の選択を学ぶ

`style-learn`は明示した修正前後の文章と既存の`profile`を受け取り、更新した`profile`を返します。`style-review`はその設定と文章を照合し、変更候補と原文の位置を返します。CLI・JSON dispatch・MCPから利用できます。CLIのJSON入力全体は2MiBまでで、本文自体の128KiB上限とは分けています。敬語・ため口や作者らしさは採点しません。

```sh
godd-unslop style-learn examples.json
godd-unslop style-review review.json
```

`examples.json`の入力例:

```json
{"examples":[{"before":"記録を保存しました。次に確認します。","after":"記録を保存しました。\n次に確認します。"}]}
```

学習結果の`profile`を利用者側で保存し、次回の入力に渡してください。`style-learn`には`examples`と任意の`profile`、`style-review`には`text`と`profile`を指定します。dispatchのactionは`style_learn`と`style_review`、MCPツールは`unslop_style_learn`と`unslop_style_review`です。

学習では修正箇所の前後の文字列を手がかりにします。同じ局所的な語句の並びが別の文章に現れた場合にも照合しますが、同じ意味の言い換え全般へは適用しません。相反する修正例は競合として返し、勝手に片方を選びません。

改行は文末というだけで一律に挿入するものではありません。つながる語句・節・文をまとめ、改行と空行によって読む間隔を変えるという用途を、修正例で示します。形態素・構文・談話の意味解析を実装したものではなく、未観測の意味のつながりを理解したとは判定しません。自動で本文を書き換えたり、会話を収集したり、外部へ送信したりしません。文体設定には入力した語句が含まれるため、個人の設定・修正例を公開リポジトリへコミットしないでください。
