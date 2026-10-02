# command_run 範例

## 什麼時候用

只用來執行程式：建置、測試、安裝、執行。
檔案與 Git 操作有專用工具，不要用 `command_run` 代替。

## 基本呼叫

`{"id":"call_001","tool":"command_run","arguments":{"executable":"powershell","arguments":["-NoProfile","-Command","cargo check"],"cwd":"D:/proj"}}`

要點：

- `arguments` 是陣列。
- `cwd` 一定要填，用專案資料夾。
- 指令保持簡短，一個呼叫只做一件事。
- 不要把大段程式碼放進 `-Command`。

## 適合的用法

- 建置：`cargo check`、`cargo build --release`
- 測試：`cargo test -p <套件名稱>`
- 前端：`npm run build`
- 安裝：`npm install`

## 不要這樣用（改用專用工具）

| 想做的事 | 不要用 | 改用 |
|---|---|---|
| 讀檔 | `Get-Content` | 讀檔類工具 |
| 搜尋內容 | `Select-String` | 搜尋類工具 |
| 看 Git 狀態或差異 | `git status`、`git diff` | Git 類工具 |
| 修改檔案 | PowerShell 的 `-replace` | 取代文字類工具 |

## 結果怎麼看

- 看結束碼與輸出。結束碼不是 0，代表指令失敗：讀錯誤輸出，找原因，改指令再試，最多重試兩次。
- 被拒絕（`policy_denied`）：停止，回報，不要換方式繞過。
- 輸出太長：縮小範圍，例如只跑某個套件的測試。
