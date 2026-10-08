# shell_wait

## 用途

等待已建立的 ChatCMD 持久終端執行狀態更新。與 `shell_read` 不同，`shell_wait` 主要用來等待終端命令完成或等待一段時間後取得目前狀態。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"shell_wait","note":"等待持久終端狀態更新｜使用已建立的 terminal session","arguments":{"sessionId":"<terminal-session-id>"}}
```

## 必填參數

- `sessionId`：目標持久終端的 terminal session ID。必須使用 `shell_create` 或 `shell_list` 取得的實際 session ID。

目前已實測 `{}` 會回報缺少 `sessionId`；提供有效的 terminal session ID 後可以成功執行。

## 實測結果

以已建立的 PowerShell terminal session 實際呼叫：

```chatcmd_tool_call
{"id":"call_448","tool":"shell_wait","note":"探測 shell_wait 欄位｜已取得有效 terminal sessionId，繼續取得其餘參數","arguments":{"sessionId":"<terminal-session-id>"}}
```

實測成功，回應包含：

```text
{
  "completed": false,
  "exitCode": null,
  "lastSequence": 4,
  "sessionId": "<terminal-session-id>",
  "usage": {
    "elapsedMs": 30033,
    "outputBytes": 0
  },
  "waitTimedOut": true
}
```

本次實測顯示等待約 30 秒後 `waitTimedOut` 為 `true`，終端仍未完成，因此 `completed` 為 `false`、`exitCode` 為 `null`。

## 使用流程

1. 使用 `shell_create` 建立持久終端。
2. 使用 `shell_write` 送入命令。
3. 使用 `shell_wait` 等待狀態更新。
4. 使用 `shell_read` 取得終端輸出。
5. 依 `completed`、`exitCode` 與 `waitTimedOut` 判斷是否需要繼續等待或讀取輸出。

注意：不要把 ChatCMD 回合的 `sessionId` 與 terminal session 的 `sessionId` 混用。

## 注意事項

- 不要猜測 `sessionId`；使用 `shell_create` 或 `shell_list` 的實際結果。
- `waitTimedOut: true` 不代表終端命令失敗，只表示這次等待沒有等到完成。
- `completed: false` 時不要把命令視為已完成。
- `exitCode: null` 表示目前尚未取得完成時的結束碼。
- 本範例只記錄目前已實測的 `sessionId` 呼叫與回應欄位；其他可選參數應以實際工具目錄定義為準。
