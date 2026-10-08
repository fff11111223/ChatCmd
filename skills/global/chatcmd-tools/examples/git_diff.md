# git_diff

## 用途

查看目前 Git 工作區相對於基準版本的內容差異。這是唯讀操作，適合在修改後確認實際變更。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "git_diff",
  "arguments": {}
}
```

## 參數

- 不需要提供參數。已由實際呼叫確認 `{}` 可以直接執行。

## 實際輸出

成功結果會包含執行資訊與 diff 文字，例如：

- `exitCode`：Git 執行結果。
- `stdout`：unified diff 內容。
- `stderr`：Git 的錯誤或診斷輸出。
- `stdoutBytes` / `artifactBytes`：輸出大小。
- `truncated`：輸出是否被截斷。

Diff 內容通常包含：

- `diff --git`：變更檔案。
- `---` / `+++`：舊版與新版檔案。
- `@@`：變更所在的行範圍。
- `+`：新增內容。
- `-`：刪除內容。

## 使用方式

1. 修改前可先用 `git_status` 確認工作區狀態。
2. 修改後使用 `git_diff` 檢查實際內容變更。
3. 若輸出過大，先縮小範圍或改用適合的 Git 查詢方式，不要假設截斷內容代表完整 diff。
4. `git_diff` 只查看差異，不會提交或修改 Git 歷史。

## 已驗證行為

- `{}` → 成功，`exitCode: 0`。
- 實際結果輸出 12820 bytes 的 unified diff。
- 本次結果包含 Rust 原始碼、resident.md 等目前工作區變更。

## 注意

`git_diff` 是查看差異的工具；不要把它當成 `git_status` 或 `git_commit` 使用。
