# fs_restore_quarantine

## 用途

從隔離區還原檔案到指定目的地。核心規則指出此工具只在使用者明確要求處理隔離區時使用。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "fs_restore_quarantine",
  "arguments": {
    "quarantinePath": "D:\\quarantine\\item",
    "destination": "D:\\project\\restored-item"
  }
}
```

## 參數

- `quarantinePath`：隔離區項目的路徑。已由實際呼叫確認為必填。
- `destination`：還原後的目的路徑。已由實際呼叫確認為必填。

## 使用方式

1. 只有使用者明確要求還原隔離項目時才使用。
2. 還原前確認 `quarantinePath` 與 `destination` 都是使用者要求的目標。
3. 不要用其他工具繞過隔離區處理的權限限制。
4. 本範例只示範參數結構；沒有對實際隔離項目執行還原。

## 已驗證行為

- `{}` → `invalid_arguments`，缺少 `quarantinePath`。
- 只提供 `quarantinePath` → `invalid_arguments`，缺少 `destination`。

## 注意

這是具狀態變更的隔離區還原操作。若使用者只是要求調查隔離區內容，不應直接執行還原。
