# shell_signal

## 用途

向指定的 PTY 工作階段送出可攜式 terminal signal。常用於中斷正在執行的互動式命令；例如 `CtrlC` 可用於要求程序優雅中斷。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "shell_signal",
  "arguments": {
    "sessionId": "<session-id>",
    "signal": "CtrlC"
  }
}
```

## 參數

- `sessionId`：必填，指定要送出 signal 的 PTY session。
- `signal`：必填，指定要送出的 terminal signal。
- `CtrlC`：已由實際測試確認可作為 `signal` 使用，用於中斷目前的互動式命令。

## 使用方式

1. 先使用 `shell_create` 建立 PTY session，取得 `sessionId`。
2. 使用 `shell_signal` 對該 session 發送 signal。
3. 需要確認後續狀態時，再使用 `shell_read` 或 `shell_wait` 讀取 terminal 狀態與輸出。

## 常見錯誤

- 缺少 `sessionId` 或 `signal` 會造成參數驗證失敗。
- 不應自行猜測 `sessionId`；應使用實際建立的 PTY session ID。
- `shell_signal` 屬於停止／清理類操作，應遵循目前的安全政策與權限限制。

## 注意

`shell_signal` 是對既有 PTY session 發送 signal，不是建立新的程序，也不是直接以程序 ID 終止程序。

## 已驗證

- 工具註冊型別為 `ShellSignalArgs`。
- `sessionId` 與 `signal` 均為必填字串欄位。
- `CtrlC` 已由測試確認為可接受的 signal alias。
