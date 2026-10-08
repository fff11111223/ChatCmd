# git_status

## 用途

查看目前 Git 工作區、分支與檔案狀態。這是唯讀操作，適合在修改前後確認工作區狀態。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "git_status",
  "arguments": {}
}
```

## 參數

- 不需要提供參數。已由實際呼叫確認 `{}` 可以直接執行。

## 實際輸出

成功結果包含 Git 狀態資訊，例如：

- `branch.oid`：目前 HEAD 的 commit。
- `branch.head`：目前分支名稱。
- `branch.upstream`：追蹤的 upstream 分支。
- `branch.ab`：相對 upstream 的 ahead / behind 數量。
- 檔案狀態：包含 `modified` 與 `untracked` 等項目。
- `structured.data.branch`：結構化的分支資訊。
- `structured.data.entries`：結構化的檔案狀態清單。

## 使用方式

1. 修改檔案前先查看工作區，避免誤把既有修改當成自己的修改。
2. 修改完成後再次執行，確認新增、修改或未追蹤檔案符合預期。
3. 若要查看具體內容差異，使用 `git_diff`。
4. `git_status` 本身不會提交或修改 Git 歷史。

## 已驗證行為

- `{}` → 成功，`exitCode: 0`。
- 實際結果包含 `branch.head: master`、`branch.upstream: origin/master`，以及工作區檔案狀態。

## 注意

`git_status` 只負責查看狀態；不要把它當成 `git_diff` 或 `git_commit` 使用。
