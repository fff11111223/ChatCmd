# shell_create

## 用途

建立一個由 ChatCMD 管理的持續終端 session，適合互動式工作或需要持續執行的程序。與一次性執行的 `command_run` 不同，建立後可使用 `shell_write`、`shell_wait`、`shell_read` 等工具繼續操作。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"shell_create","note":"建立持續終端｜使用預設終端設定","arguments":{}}
```

## 參數

本次實際呼叫 `{}` 即成功建立終端，因此沒有必填參數需要指定。成功回應會提供後續操作需要的 `sessionId`。

## 成功回應

本次驗證成功建立 PowerShell 終端，回應包含：

```json
{
  "columns": 120,
  "createdAtUnixMs": 1791352685137,
  "executable": "powershell.exe",
  "exitCode": null,
  "initialWorkingDirectory": "D:\\frank\\gemini\\ChatCmd",
  "lastSequence": 1,
  "processId": 7748,
  "rows": 30,
  "sessionId": "33af25e6-b0c5-450a-9575-005ff4a0ef58",
  "status": "running"
}
```

實際的 `processId`、`sessionId`、時間與工作目錄會依當次建立結果而不同。

## 使用方式

1. 呼叫 `shell_create` 建立終端。
2. 保存成功回應中的 `sessionId`。
3. 使用該 `sessionId` 呼叫其他 shell 工具。
4. 不需要終端時，依使用者要求再處理終端生命週期。

## 注意

- `shell_create` 會實際建立持續執行的終端程序。
- 預設成功建立的是 `powershell.exe`。
- 不要把一般 ChatCMD 回合的 `sessionId` 當成終端 session；應使用 `shell_create` 回應中的 `sessionId`。
- 本次未指定參數即可成功建立，因此不要自行添加未驗證的參數。
