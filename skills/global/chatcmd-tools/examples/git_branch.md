# git_branch

## 用途

列出 Git repository 的 branches，並提供可供 Agent 使用的結構化 branch 資訊。支援分頁，適合 repository branch 較多時逐頁讀取。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "git_branch",
  "arguments": {
    "cwd": "D:\\workspace\\project"
  }
}
```

## 參數

- `cwd`：選填，指定 Git repository 的工作目錄。
- `limit`：選填，限制單次回傳的 branch 數量。
- `cursor`：選填，使用前一次結果提供的 signed cursor 取得下一頁。
- `path`：舊版相容欄位，可作為 `cwd` alias；新範例優先使用 `cwd`。

## 結構化結果

每個 branch entry 包含：

- `name`：branch 名稱。
- `objectId`：branch 所指向的 Git object ID。
- `current`：是否為目前 checkout 的 branch。
- `upstream`：追蹤的 upstream branch；沒有 upstream 時可能沒有此值。

結果另包含分頁資訊：

- `nextCursor`：仍有下一頁時提供的 signed cursor。
- `hasMore`：是否還有更多 branch。

## 分頁使用方式

第一次呼叫可以設定 `limit`：

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "git_branch",
  "arguments": {
    "cwd": "D:\\workspace\\project",
    "limit": 20
  }
}
```

若結果的 `hasMore` 為 `true`，將 `nextCursor` 原值帶入下一次呼叫：

```chatcmd_tool_call
{
  "id": "call_002",
  "tool": "git_branch",
  "arguments": {
    "cwd": "D:\\workspace\\project",
    "limit": 20,
    "cursor": "<nextCursor>"
  }
}
```

不要自行修改或產生 cursor；應使用工具前一次結果提供的 signed cursor。

## 使用建議

- 需要知道目前 branch 時，查看 `current`。
- 需要判斷 branch 是否追蹤遠端時，查看 `upstream`。
- branch 數量較多時使用 `limit` 與 `nextCursor` 分頁，不要假設一次結果包含全部 branch。
- `git_branch` 是唯讀操作，不會建立、刪除或切換 branch。

## 已驗證

- 工具註冊型別為 `CwdArgs`。
- `cwd` 為選填欄位，另支援 `limit` 與 signed `cursor`。
- `git_branch` 的結構化資料包含 branch name、object ID、current 與 optional upstream。
- 分頁結果使用 `nextCursor` 與 `hasMore`。
