# 管理元別の一覧と検出対象

開発環境の一覧は、mise・uv・rustup・その他の管理元ごとに別の表を表示します。WinGetのアプリは表示を有効にした場合だけ、専用の表に表示します。検索はすべての分類に適用されます。

## 追加した検出経路

| 対象                     | 取得方法                                                             | 表示する情報                                                       |
| ------------------------ | -------------------------------------------------------------------- | ------------------------------------------------------------------ |
| 旧Pythonランチャー（py） | `py --list-paths`                                                    | 登録タグ、既定マーク、Python実行先                                 |
| Python Install Manager   | `pymanager list --format=json`                                       | 管理版と外部登録を区別。版、実行先、登録タグ                       |
| pyenv                    | `pyenv versions --bare`、`pyenv prefix <版>`                         | 導入版、配置先。systemは所有版として登録しない                     |
| Conda                    | `conda env list --json`、各環境の`conda list --prefix <環境> --json` | 環境ごとのPython・Node・Go。環境を有効化せずメタデータを参照       |
| fnm                      | `fnm list`、指定版のNodeに`process.execPath`を問い合わせ             | 導入版、実行先。導入・use・既定版の変更はしない                    |
| nvm-windows              | `nvm list`、`nvm root`                                               | 導入版、問い合わせたルート配下で確認できるNode実行先               |
| PATH上の主要ランタイム   | Python・Node・Java・Go・.NET・Bun・Denoの版を問い合わせ              | 選ばれる実行ファイルと版。Python・Nodeはshimの先の実体も問い合わせ |

追加対象は一覧取得に対応しています。更新・削除の自動実行に対応したことを意味しません。画面では「一覧取得のみ」と表示します。mise・uv・rustupなど、既存の更新経路は維持しています。

旧pyはPythonを選ぶランチャーであり、導入元とは限りません。「Python登録情報（py）」として表示し、インストーラーや更新元を断定しません。同じ実体がuv・miseなどで管理されていると確認できた場合、管理元の表へ集約し、別経路の検出情報は詳細に残します。同じ版でも別の環境・配置先は別の参照として扱います。

## 配置の取得

- OSホーム、PATH、管理ツールからの応答、環境変数、設定画面の明示パスを利用します。特定のユーザー名・ドライブ・WSL名を前提にしません。
- pyはPATHに加え、OSから得たWindowsディレクトリとローカルアプリデータ配下の標準配置を候補として調べます。これらは検出用の候補であり、配置の強制ではありません。
- Condaは`CONDA_EXE`も利用します。PATH外のツールは設定画面で対象環境と実行ファイルを指定できます。
- WindowsのPython実行エイリアスは、インストーラーを開かないよう直接実行せず、py／pymanagerの登録情報を参照します。
- 未検出、取得失敗、版・配置の未取得を区別します。旧pyのタグだけから完全なパッチ版を作りません。

## 範囲と制限

py／pymanager／nvm-windowsはWindowsで検出します。pyenv・Conda・fnmとPATH調査はWindows／WSLの実行環境を分けて扱います。

シェル関数だけの管理ツール、Windowsのバッチファイル経由の管理元、Unix版nvmのシェル初期化、すべてのインストーラー・全SDK・アプリ同梱ランタイムを網羅するものではありません。Windowsのpyenv-winはバッチ形式のため、このアダプターでの直接呼び出しは対象外です。Condaは実行ファイル本体を指定してください。Homebrew・Scoop・Chocolateyの全パッケージ一覧の取り込みも未実装です。

Condaは32環境、pyenv・Node管理元は100版を超える場合、完了扱いにせず取得失敗として報告します。取得できた管理一覧はインストール元の証拠ですが、全プロジェクトの動作確認や未使用の証明ではありません。

追加6経路は、一時ディレクトリの模擬実行ファイルと実際のバックエンドを使って検証します。管理ツールをこのPCへ新しくインストールする試験ではありません。

## 参照した公式仕様

- [Python on Windows](https://docs.python.org/3/using/windows.html)
- [Python Install Managerの一覧出力](https://github.com/python/pymanager/blob/main/src/manage/list_command.py)
- [pyenvのコマンド](https://github.com/pyenv/pyenv/blob/master/COMMANDS.md)
- [Condaの環境一覧](https://docs.conda.io/projects/conda/en/stable/commands/env/list.html)
- [fnmのコマンド](https://github.com/Schniz/fnm/blob/master/docs/commands.md)
- [nvm-windowsのコマンド](https://github.com/coreybutler/nvm-windows#usage)
