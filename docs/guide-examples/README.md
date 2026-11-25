# 例文と入力ファイル

`article.txt` は領収書の保存先を頼む文です。`revised.txt` には、書き手が保存先の名前を確かめて直した文が入っています。

このフォルダーで次のコマンドを実行してください。

```sh
godd-unslop analyze article.txt
godd-unslop compare article.txt revised.txt
```

`recipient-request.json` に入っているのは依頼文だけです。\
`context-for` は、`recipient.json` を使って端末のデータベースに登録した相手の設定を読み込みます。

[操作の手順](../user-guide.md) · [図解ガイド](../user-guide.pptx)
