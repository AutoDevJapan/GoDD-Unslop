# 例文と入力ファイル

`article.txt` は領収書の保存先を頼む文です。`revised.txt` には、書き手が保存先の名前を確かめて直した文が入っています。

このフォルダーで次のコマンドを実行してください。

```sh
godd-unslop analyze article.txt
godd-unslop compare article.txt revised.txt
```

`recipient-request.json` に入っているのは依頼文だけです。`context-for` は、端末のデータベースから登録済みの相手の設定を読み込みます。

架空の同僚 `colleague-1` を登録する前に、環境変数 `GODD_UNSLOP_DB` に、リポジトリの外に作る練習用DBファイルの絶対パスを指定してください。\
登録・確認・削除には同じデータベースを使ってください。

[操作の手順](../user-guide.md) · [図解ガイド](../user-guide.pptx)
