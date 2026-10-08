# fs_quarantine_gc

## 用途

處理隔離區的清理操作。核心規則指出此工具只在使用者明確要求處理隔離區時使用。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "fs_quarantine_gc",
  "arguments": {
    "path": "D:\\quarantine\\item"
  }
}
```

## 參數

- `path`：隔離區項目的路徑。已由實際呼叫確認為必填。

## 使用方式

1. 只有使用者明確要求進行隔離區清理時才使用。
2. 執行前確認 `path` 是使用者要求處理的隔離區目標。
3. 不要用其他工具繞過隔離區處理的權限限制。
4. 本範例只示範參數結構；沒有對實際隔離區項目執行清理。

## 已驗證行為

- `{}` → `invalid_arguments`，缺少 `path`。
- 提供不存在的 `path` → `not_found`，Windows `os error 2`。

## 注意

這是隔離區處理操作。若使用者只是要求調查隔離區內容，不應直接執行清理。
