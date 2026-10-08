# shell_read

## 用途

讀取 ChatCMD 管理的持續終端輸出。它讀的是已存在的 terminal session，不會建立新的終端。

與 `shell_wait` 不同：
- `shell_wait` 用來等待終端是否完成。
- `shell_read` 用來取得終端已產生的輸出事件。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"shell_read","note":"讀取終端輸出｜取得指定 terminal session 的輸出事件","arguments":{"sessionId":"<terminal-session-id>"}}
```

## 參數

- `sessionId`：必填。必須是 `shell_create` 或 `shell_list` 取得的 terminal session ID。

不要把 ChatCMD 回合的 `sessionId` 當成 terminal session ID。

## 實測成功結果

以已建立的 PowerShell terminal session 實測成功，回應包含：

```text
{
  "droppedBytes": 0,
  "droppedEvents": 0,
  "events": [
    {
      "data": "...",
      "encoding": "utf-8",
      "eventType": "output",
      "sequence": 1,
      "stream": "pty",
      "timestampUnixMs": 1791352685282
    }
  ],
  "latestAvailableSequence": 4,
  "oldestAvailableSequence": 1,
  "replayTruncated": false,
  "sessionId": "<terminal-session-id>",
  "usage": {
    "bytesRead": 426,
    "outputBytes": 426
  }
}
```

實際完整回應另外還可能包含 `finalizer`、`requiresFinalization`、`taskId`、`turnId` 等回合資訊。

## events

每個輸出事件可包含：

- `data`：終端輸出的內容，可能包含 ANSI escape sequence。
- `encoding`：實測為 `utf-8`。
- `eventType`：實測輸出事件為 `output`。
- `sequence`：輸出事件的序號。
- `stream`：實測為 `pty`。
- `timestampUnixMs`：事件時間戳。

## 使用方式

通常流程是：

1. 用 `shell_create` 建立持續終端。
2. 保存回傳的 terminal `sessionId`。
3. 用 `shell_write` 輸入指令。
4. 視需要用 `shell_wait` 等待。
5. 用 `shell_read` 讀取輸出事件。

如果需要從特定輸出位置繼續讀取，應依工具目錄中實際支援的欄位使用，不要自行猜測參數。

## 錯誤

已驗證：

- `{}` → `invalid_arguments`：缺少 `sessionId`。
- 使用不存在的 terminal session ID → `session_not_found`。

## 注意

- `shell_read` 是唯讀輸出，不會建立、關閉或修改終端。
- `data` 可能包含 ANSI 控制序列，因此不要假設內容一定是純文字。
- 若 `droppedBytes` 或 `droppedEvents` 非 0，表示部分歷史輸出可能已被丟棄。
- 若 `replayTruncated` 為 `true`，表示可重播的輸出不是完整歷史。
