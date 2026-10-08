# git_log

## 用途

查看 Git 提交歷史。這是唯讀操作，適合了解近期提交、作者、時間與提交訊息。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "git_log",
  "arguments": {}
}
```

## 參數

- 不需要提供參數。已由實際呼叫確認 `{}` 可以直接執行。

## 實際輸出

成功結果包含執行資訊與提交清單，例如：

- `exitCode`：Git 執行結果。
- `stdout`：Git log 的分隔格式輸出。
- `structured.data.entries`：結構化提交清單。
- `structured.data.entries[].commit`：完整 commit SHA。
- `structured.data.entries[].shortCommit`：短版 commit SHA。
- `structured.data.entries[].author`：作者。
- `structured.data.entries[].authoredAt`：提交時間。
- `structured.data.entries[].subject`：提交訊息標題。
- `structured.data.hasMore`：是否還有未返回的歷史紀錄。
- `structured.data.nextCursor`：下一頁游標；若 `hasMore` 為 false 則通常為 `null`。

## 使用方式

1. 需要了解近期變更時，可先使用 `git_log` 查看提交歷史。
2. 已知 commit SHA 後，可使用 `git_show` 查看該提交的詳細內容。
3. 若只想知道目前工作區有哪些未提交修改，使用 `git_status`。
4. `git_log` 只查看歷史，不會修改或提交 Git 歷史。

## 已驗證行為

- `{}` → 成功，`exitCode: 0`。
- 實際結果包含 `structured.data.entries`，本次返回 19 筆提交紀錄。
- `structured.data.hasMore` 為 `false`，`nextCursor` 為 `null`。

## 注意

`git_log` 顯示的是提交歷史，不等同於目前工作區的未提交差異；需要內容差異時使用 `git_diff`。
