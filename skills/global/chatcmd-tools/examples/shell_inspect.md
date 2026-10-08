# shell_inspect

## 用途

查看單一 ChatCMD 管理終端的狀態與資訊。與 `shell_list` 不同，`shell_inspect` 需要指定一個終端 session。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"shell_inspect","note":"查看指定終端｜需要有效的終端 sessionId","arguments":{"sessionId":"<terminal-session-id>"}}
```

## 參數

- `sessionId`：必填，要查詢的終端 session ID。

## 使用方式

先使用 `shell_list` 取得目前由 ChatCMD 管理的終端，再把其中有效的終端 session ID 傳給 `shell_inspect`。不要把一般 ChatCMD 回合的 `sessionId` 當成終端 session。

## 已驗證行為

空參數會得到：

```text
[invalid_arguments] missing field `sessionId`
```

使用不存在的終端 session ID 會得到：

```text
[session_not_found] terminal session was not found
```

本次環境的 `shell_list` 回傳空清單，因此沒有可用的終端 session 可進一步驗證成功回應格式。

## 注意

- `shell_inspect` 是查詢工具，不會建立終端。
- 如果沒有有效的終端 session，先使用 `shell_list` 確認。
- 不要猜測 session ID，也不要把其他工具回傳的 `sessionId` 直接當成終端 session ID。
