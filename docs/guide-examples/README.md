# 操作を試すサンプル

このフォルダーには、[図解ガイド（PowerPoint）](../user-guide.pptx)と[文章版ガイド](../user-guide.md)の操作例で使うUTF-8ファイルを格納しています。

| ファイル | 使い方 |
| --- | --- |
| `article.txt` | `godd-unslop analyze article.txt`で指摘を確認します。 |
| `revised.txt` | `godd-unslop compare article.txt revised.txt`で原文と比べます。 |
| `plan-request.json` | `godd-unslop dispatch plan-request.json`で用途付きの推敲手順を確認します。 |
| `learn-request.json` | `godd-unslop style-learn learn-request.json`で改行と空行の修正例を学習します。 |
| `rewrite-request.json` | `godd-unslop rewrite rewrite-request.json`で架空の確認報告をAIで改稿します。ChatGPTログインが必要です。 |
| `recipient.json` | `godd-unslop relationship-put recipient.json`で架空の同僚の設定を保存します。 |
| `recipient-request.json` | `godd-unslop context-for colleague-1 recipient-request.json`で保存した相手の設定を確認します。`rewrite-for`での改稿はAIへ送信します。 |
| `style-review-request.json` | `godd-unslop style-review style-review-request.json`で、文章と保存設定を渡す形を試します。 |

実行ファイルのインストール後、これらのファイルを自分の作業フォルダーへコピーして使ってください。  
設定の保存・再利用・追加の手順は[文章版ユーザーガイド](../user-guide.md#改行と空行を修正例から学ぶ)にあります。

試しに登録した架空の同僚が不要になったら、次のコマンドで削除できます。`colleague-1`が今回のサンプルで登録した相手であることを、削除前に確認してください。

```sh
godd-unslop relationship-get colleague-1
godd-unslop relationship-delete colleague-1
```

サンプルは公開用の架空の例文です。  
個人の文章や保存した`profile`は、公開リポジトリへ追加しないでください。
