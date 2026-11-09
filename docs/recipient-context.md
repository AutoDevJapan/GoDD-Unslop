# 相手と場面に合わせて書く

同じ依頼でも、親しい同僚へのチャットと、社外へ提出する報告書では書き方が変わります。  
GoDD Unslopには、相手との関係や文章の用途を渡して、内容を変えずに伝え方を調整できます。

役職だけで口調を決めるわけではありません。  
部下でもまだ親しくない相手には丁寧に、気心の知れた上司には親しみを交えた敬語で、といった調整に使います。

## 相手の設定を保存する

次のサンプルをUTF-8の`recipient.json`に保存します。実在する人の情報ではありません。

```json
{
  "id": "colleague-1",
  "label": "架空の同僚",
  "confirmed": true,
  "source": "本人が確認した設定",
  "context": {
    "audience": "同じ部署の親しい同僚",
    "recipient": {"relation": "peer", "closeness": "close"},
    "tone": {"formality": 2, "warmth": 3, "directness": 2, "emoji": "light"},
    "channel": "chat"
  }
}
```

```sh
godd-unslop relationship-put recipient.json
godd-unslop relationship-get colleague-1
godd-unslop relationship-list
```

`id`は英数字・ハイフン・下線で付ける識別名、`label`は自分が見分けるための名前です。  
`confirmed`は内容を確認済みかどうか、`source`は何をもとに設定したかを記録します。未確認の相手の設定は、改稿には使いません。

保存した内容を変更する場合は、同じ`id`のJSONを直して`relationship-put`で保存し直します。  
不要になった設定は、次のコマンドで削除できます。

```sh
godd-unslop relationship-delete colleague-1
```

## 口調と絵文字を調整する

`formality`・`warmth`・`directness`は0から4で指定します。細かな語尾を機械的に置換するのではなく、AIが文章を組み立てるときの目安として使います。

| 項目 | 低い値 | 中間の値 | 高い値 |
| --- | --- | --- | --- |
| `formality` | 0〜1：親しい口語、やわらかいタメ口 | 2：フレンドリーな敬語 | 3〜4：標準的な敬語、改まった敬語 |
| `warmth` | 0：事実中心で控えめ | 2：穏やかで中立 | 4：親しみを伝える |
| `directness` | 0：婉曲な依頼 | 2：理由と依頼を自然に伝える | 4：要点を簡潔に伝える |

`relation`は`subordinate`（部下）、`peer`（同僚）、`superior`（上司）、`external`（社外）、`unknown`（未確認）から選びます。  
`closeness`は`close`（親しい）、`normal`（通常）、`distant`（まだ距離がある）で、役職とは別に指定します。

`emoji`は`none`（使わない）、`light`（控えめに使ってよい）、`match`（参考例に合わせる）です。  
許可しても毎回絵文字を付けるわけではなく、謝罪・注意・深刻な報告では場面に合うかを考慮します。

文章の用途は`channel`へ`chat`・`email`・`report`・`official`を指定できます。  
`official`や正式な文書として指定した場合は、相手の保存設定で絵文字を許可していても使いません。

## 保存した相手に向けて書き直す

次の内容をUTF-8の`request.json`へ保存します。

```json
{
  "text": "資料を火曜日17時までに送ってください。難しい場合は月曜日12時までに連絡してください。",
  "context": {"purpose": "資料の送付を依頼する"}
}
```

```sh
godd-unslop rewrite-for colleague-1 request.json
```

送信する背景を先に確認したい場合は、`rewrite-for`の代わりに`context-for`を使います。
保存した設定と今回の指定を合わせた内容だけを表示し、AIは呼び出しません。

```sh
godd-unslop context-for colleague-1 request.json
```

指定した相手1人分の設定と、今回の文章を組み合わせて改稿します。  
相手の一覧や、他の人の設定をまとめてAIへ送ることはありません。

今回だけ正式な文書にしたい場合は、リクエストに次のような指定を追加します。保存した相手の設定自体は変わりません。

```json
{
  "text": "資料を火曜日17時までに送ってください。難しい場合は月曜日12時までに連絡してください。",
  "context": {
    "channel": "official",
    "tone": {"formality": 4},
    "purpose": "正式な依頼文を作成する"
  }
}
```

丁寧さを変えても、火曜日17時という期限や、間に合わない場合の連絡期限は変えません。  
結果を読み、相手との普段のやり取りに合っているかを確認してから使ってください。このコマンドがメールやチャットを送信することはありません。

## 背景が足りない場合

Skillを使うAIは、まず依頼内容、提供された資料、参照を許可された過去の文書を調べます。  
それでも重要な点が分からなければ、質問してから文章を作成します。

CLIは手元の会話や他の文書を勝手に調べません。背景は`context`へ渡してください。  
分かっていない重要事項を`missing_facts`へ指定すると、改稿せずに`needs_context`と質問を返します。

会社独自の文体を必ず使う必要がある場合は、`company_style_required`を`true`にし、`company_style`か`reference_examples`を渡します。  
一般的な敬語で十分な文書に、会社独自のルールを作り足すことはしません。

## 関係性DBの保存先

Windowsでは通常、`%LOCALAPPDATA%\godd-unslop\relationships.sqlite3`へ保存します。  
macOS・Linuxでは`$XDG_DATA_HOME/godd-unslop/relationships.sqlite3`、未設定なら`$HOME/.local/share/godd-unslop/relationships.sqlite3`です。

変更する場合は、`GODD_UNSLOP_DB`へ非公開フォルダーの絶対パスを指定してください。Gitリポジトリの中には保存できません。

DBは端末内のファイルであり、この機能がクラウドへ同期することはありません。  
ただし`rewrite-for`を実行すると、指定した相手の背景情報と今回の文章は、改稿に使うAIサービスへ送られます。実在する人の情報や参考文書は、利用してよい範囲を確認して登録してください。

DBそのものを暗号化する機能はありません。OSのアカウントとファイル権限で管理し、公開リポジトリへ追加しないでください。

ビルド時には同梱のSQLiteもコンパイルするため、Rustに加えてCのビルド環境が必要です。
