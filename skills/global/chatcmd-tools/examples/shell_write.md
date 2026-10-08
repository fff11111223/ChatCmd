# shell_write

## 用途

向已建立的 ChatCMD 持久終端寫入文字或命令。與 `command_run` 不同，`shell_write` 會把輸入送進既有的互動式終端 session。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"shell_write","note":"向持久終端送入命令｜使用已建立的 terminal session","arguments":{"sessionId":"<terminal-session-id>","text":"echo CHATCMD_SHELL_WRITE_PROBE"}}
```

## 必填參數

- `sessionId`：目標持久終端的 session ID。必須使用 `shell_create` 或 `shell_list` 取得的 terminal session ID。
- `text`：要寫入終端的文字或命令。

## 實測結果

本工具已實際以有效 PowerShell terminal session 呼叫：

```chatcmd_tool_call
{"id":"call_443","tool":"shell_write","note":"探測 shell_write 欄位｜已確認 sessionId，現在補 text 取得其餘要求","arguments":{"sessionId":"<terminal-session-id>","text":"echo CHATCMD_SHELL_WRITE_PROBE"}}
```

呼叫成功，回應包含：

```text
{
  "writtenBytes": 32
}
```

這證明 `sessionId` 與 `text` 是目前已驗證可用的必要欄位。

## 使用流程

1. 使用 `shell_create` 建立持久終端。
2. 保存回應中的 terminal `sessionId`。
3. 使用 `shell_write` 將命令或輸入送入該 session。
4. 使用 `shell_read` 讀取終端輸出；需要等待時使用 `shell_wait`。

注意：不要把 ChatCMD 回合的 `sessionId` 與 terminal session 的 `sessionId` 混用。若 session 不存在，應先用 `shell_list` 確認目前有效的終端。

## 與 command_run 的區別

- `command_run`：一次性執行命令，適合建置、測試、安裝等非互動工作。
- `shell_write`：向已存在的持久終端送入輸入，適合需要維持 shell 狀態的互動流程。

## 注意事項

- 不要猜測 `sessionId`；使用 `shell_create` 或 `shell_list` 的實際結果。
- `shell_write` 本身只負責寫入，不代表命令已完成；完成狀態應透過 `shell_read` 或 `shell_wait` 確認。
- 本範例只驗證 `sessionId` 與 `text`；其他可能的行為應以實際工具目錄定義為準。
