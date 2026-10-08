# shell_resize

調整既有終端 session 的終端尺寸。

## 用途

`shell_resize` 用於調整指定 terminal session 的欄數與列數。它需要一個仍在執行中的 shell session，以及新的 `columns`、`rows` 尺寸。

## 參數

- `sessionId`：要調整尺寸的 terminal session ID，必填。
- `columns`：終端欄數，必填。
- `rows`：終端列數，必填。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"shell_resize","note":"調整終端尺寸｜設定指定 session 的欄數與列數","arguments":{"sessionId":"<terminal-session-id>","columns":120,"rows":30}}
```

## 實際成功結果

本次使用既有 PowerShell terminal session，以 `120×30` 尺寸測試成功：

```json
{
  "columns": 120,
  "createdAtUnixMs": 1791352685137,
  "executable": "powershell.exe",
  "exitCode": null,
  "finalizer": "agent_turn_complete",
  "initialWorkingDirectory": "\\\\?\\D:\\frank\\gemini\\ChatCmd",
  "lastSequence": 4,
  "processId": 7748,
  "requiresFinalization": true,
  "rows": 30,
  "sessionId": "<terminal-session-id>",
  "status": "running",
  "taskId": "<task-id>",
  "turnId": "<turn-id>"
}
```

成功結果會回傳調整後的 `columns`、`rows`，以及該 terminal session 的狀態資訊。

## 參數錯誤案例

呼叫時省略 `rows`：

```chatcmd_tool_call
{"id":"call_001","tool":"shell_resize","note":"驗證必要參數｜故意省略 rows 以確認參數要求","arguments":{"sessionId":"<terminal-session-id>","columns":120}}
```

實際結果：

```text
[invalid_arguments] missing field `rows`
```

補上 `rows` 後即可成功執行。

## 使用流程

1. 先透過 `shell_create` 建立 terminal session，或取得既有的 `sessionId`。
2. 使用 `shell_resize` 傳入 `sessionId`、`columns`、`rows`。
3. 再使用 `shell_read` 讀取終端輸出；如需等待程序完成則使用 `shell_wait`。

## 注意事項

- `sessionId` 必須是 terminal session 的 ID；不能使用其他 ChatCMD session ID。
- `columns` 與 `rows` 都是必要欄位。
- 此工具只調整終端顯示尺寸，不是建立或關閉 terminal session。
