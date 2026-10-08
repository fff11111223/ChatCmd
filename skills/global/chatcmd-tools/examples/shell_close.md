# shell_close

## 用途

`shell_close` 用來關閉既有的 PTY shell session。

必要參數是 `sessionId`；如一般關閉無法完成，可明確指定 `force: true` 強制關閉。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"shell_close","arguments":{"sessionId":"<session-id>"}}
```

## 強制關閉

```chatcmd_tool_call
{"id":"call_002","tool":"shell_close","arguments":{"sessionId":"<session-id>","force":true}}
```

## 參數

- `sessionId`：要關閉的 PTY session ID，必填。
- `force`：是否強制關閉，選填；未指定時使用工具預設行為。

## 使用方式

1. 先透過 `shell_list` 或其他 shell 工具取得有效的 `sessionId`。
2. 一般情況先使用不帶 `force` 的 `shell_close`。
3. 只有在需要明確強制終止 session 時才指定 `force: true`。
4. 關閉後不要再對同一個 session 執行後續 shell 操作，除非確認該 session 仍有效。

## 注意

`shell_close` 是針對 PTY session 的關閉操作，不是用來終止一般 Windows process；需要終止 process 時使用 `process_kill`。
