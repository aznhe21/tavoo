# isdbのサンプル

## caption

```shell
$ cargo run -p isdb --example caption -- [PATH]
$ cargo run -p isdb --example caption --superimpose -- [PATH]
```

TSに含まれる字幕や文字スーパーを出力します。

基本的には表示可能な文字だけ出力します。
CSなどの描画系命令を確認するには環境変数`RUST_LOG`に`debug`を指定します。

既定では字幕を出力しますが、フラグ`--superimpose`を与えることで字幕の代わりに文字スーパーを出力します。

## epg

```shell
$ cargo run -p isdb --example epg -- [PATH]
```

TSに含まれる番組情報を出力します。

番組情報はサービス（チャンネル）ごとに、時間の順に出力されます。

## logo

```shell
$ cargo run -p isdb --example logo -- [PATH]
$ cargo run -p isdb --example logo -- --output [DIR] [PATH]
```

TSに含まれるロゴデータをファイルに抽出します。

ロゴファイルはPNG画像として`--output`で指定されたディレクトリに保存されます。
`--output`を省略した場合はカレントディレクトリに保存されます。

また、フラグ`--raw`を付けることでPNG画像ではなく受信したそのままの生データで保存することもできます。

## services

```shell
$ cargo run -p isdb --example services -- [PATH]
$ cargo run -p isdb --example services -- --show-events [PATH]
```

TSに含まれる全サービスの情報を出力します。

サービス情報には10進数表記及び16進数表記のサービス識別が含まれます。

また、フラグ`--show-events`を指定することで同時に番組名を出力させることもできます。

## tspid

```shell
$ cargo run -p isdb --example tspid -- [PATH]
```

TSに含まれる全PIDの情報を出力します。

出力される内容は[LibISDB]のtspidinfoと完全に同じですが、こちらの方が数倍～十数倍速いようです。

[LibISDB]: https://github.com/DBCTRADO/LibISDB
