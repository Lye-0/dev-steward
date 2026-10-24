# 管理元ごとの更新対応

端末上の管理元・設定・登録情報を検出し、現在のユーザーのホームや設定先を使用する。新しいランタイムの置き場所をアプリ独自に固定しない。

| 管理元                         | 候補の取得                                                            | 計画作成                                                                                |
| ------------------------------ | --------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| mise・uv・rustup               | 既存の管理元への問い合わせ                                            | 対応する導入計画                                                                        |
| fnm                            | list-remote                                                           | 指定版のinstall。既定版の切り替えはしない                                               |
| nvm-windows                    | Node公式の全リリース索引                                              | 指定版のinstall。useは実行しない                                                        |
| pyenv                          | install --list                                                        | 指定版のinstall。global/local指定は変更しない                                           |
| Python Install Manager         | list --online --format=json                                           | 標準PythonCoreの数値タグ。install --updateで同系列の既存ランタイムが変わる場合がある    |
| Conda                          | 設定チャンネルのsearch --json（候補）。対象環境での解決は計画時に確認 | 対象prefixと版を指定。freeze-installedによるdry-run成功後に計画を作成し、解決内容を提示 |
| 旧pyランチャー                 | 登録情報のみ                                                          | 実際の導入元を確認。ランチャーを更新管理元とみなさない                                  |
| WinGet                         | upgradeの一覧を正確なIDで照合                                         | 候補行から配布版を固定した個別計画。Microsoft Storeなど別ソースは対象外                 |
| Homebrew                       | 従来のoutdatedログ取得を維持                                          | ログ表示のみ。構造化した一覧・計画は未対応                                              |
| APT／WSL                       | list --upgradableのローカルインデックス                               | 候補表示。インデックス更新・実更新はOSの管理元で行う                                    |
| OS管理・アプリ同梱・導入元不明 | 導入元を確認する案内                                                  | ランタイムを一律に移行・削除しない                                                      |

追加管理元の候補は同一major、Pythonは同一minorの安定版を優先する。問い合わせが失敗した場合、同系列のデータが得られない場合は「取得失敗」とし、「候補なし」に変換しない。シェル関数やbat/cmd経由の管理元は、実行ファイル本体を設定する必要がある。Pythonの別配布・別アーキテクチャを自動で標準版へ切り替えない。

WinGetの項目は開発ツール候補から除外し、OS・アプリに表示する。「一覧に掲載なし」は固定・版不明・表示省略・対象外を含み、すべて最新という判定ではない。取得失敗時に以前の候補を新規取得済みとして上書きしない。表の取得対象は環境調査でWinGet exportに含まれたアプリであり、Windowsアプリ全体を網羅しない。

「候補取得」「計画作成」「更新実行」は別の対応段階。実行は利用者が計画と各工程を確認した後に行う。CondaとPython Install Managerは既存環境が変わるため、導入後のプロジェクト検証が必要。追加管理元を検出できたことだけで更新成功とは表示しない。

## 検証

Rust試験は候補の系列、プレリリース除外、WinGetのID照合・省略・形式不明、APTの版を確認する。test:managersは一時SQLiteと実行を拒否する模擬管理元で、追加管理元の候補と計画、WinGetの候補保存・選択版の固定を確認する。実アプリ試験にもWinGetの一覧から計画までの経路を含める。利用者の管理ツール・アプリへの更新実行は試験に含めない。

## 公式資料

- [WinGet upgrade](https://learn.microsoft.com/en-us/windows/package-manager/winget/upgrade)
- [fnm commands](https://github.com/Schniz/fnm/blob/master/docs/commands.md)
- [pyenv commands](https://github.com/pyenv/pyenv/blob/master/COMMANDS.md)
- [Python Install Manager](https://docs.python.org/3/using/windows.html)
- [Conda install](https://docs.conda.io/projects/conda/en/stable/commands/install.html)
