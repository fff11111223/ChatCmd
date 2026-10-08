# git_show

## 用途

查看指定 Git revision 的詳細內容，包含 commit 資訊與該版本的 diff。這是唯讀操作。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "git_show",
  "arguments": {
    "revision": "5b539a81dd5cfce90f8d150310c43bf818edf59c"
  }
}
```

## 參數

- `revision`：必填，要查看的 Git revision，例如完整 commit SHA。

已由實際呼叫確認：

- `{}` → `invalid_arguments`，缺少 `revision`。
- 提供有效 commit SHA 後可成功執行。

## 實際輸出

成功結果包含執行資訊與 Git `show` 輸出，例如：

- `exitCode`：Git 執行結果；成功時為 `0`。
- `stdout`：commit 標題、作者、日期，以及該 revision 的 diff。
- `stderr`：Git 錯誤輸出；成功時通常為空。
- `stdoutBytes`：stdout 大小。
- `artifactBytes`：artifact 大小；本次實際結果為 `5613`。
- `truncated`：輸出是否被截斷。

## 使用方式

1. 先使用 `git_log` 找到需要調查的 commit SHA。
2. 使用 `git_show` 搭配該 `revision` 查看完整 commit 與 diff。
3. 若只需要目前工作區未提交的差異，使用 `git_diff`。
4. `git_show` 是唯讀操作，不會修改 Git 歷史或工作區。

## 已驗證行為

- `{}` → `[invalid_arguments] missing field \`revision\``。
- `revision: 5b539a81dd5cfce90f8d150310c43bf818edf59c` → 成功，`exitCode: 0`。
- 本次成功結果包含 commit SHA、Author、Date、commit message 與 unified diff。
- 本次 `stdoutBytes` 為 `5613`，`truncated` 為 `false`。

## 注意

`git_show` 的 `revision` 必須是實際存在的 Git revision。不要猜測不存在的 SHA；可先用 `git_log` 查詢有效 revision。
