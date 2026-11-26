# GoDD Unslop

メールや案内文を送る前に、日本語の原稿を点検するツールです。

> 今月の領収書は既定のフォルダーに保存してください。

この文で引っかかるのは「既定」です。\
保存先が「経費」フォルダーなら、その名前を書けば伝わります。

冒頭の引用文を `article.txt` にUTF-8で保存し、そのフォルダーで次のコマンドを実行してください。

```sh
godd-unslop analyze article.txt
```

指摘した箇所と理由をJSONで返します。\
このコマンドは原稿を外部に送りません。

[インストールと使い方（公式Wiki）](https://github.com/AutoDevJapan/GoDD-Unslop/wiki) · [長文のビフォー・アフター](https://github.com/AutoDevJapan/GoDD-Unslop/wiki/%E9%95%B7%E6%96%87%E3%81%AE%E6%94%B9%E7%A8%BF%E4%BE%8B)

## ライセンス

[GPLv3以降](LICENSE)
