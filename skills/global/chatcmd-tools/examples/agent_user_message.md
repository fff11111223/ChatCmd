# agent_user_message

將目前 user turn 的原始使用者訊息提交給 agent runtime。

## 呼叫

```json
{"id":"call_XXX","tool":"agent_user_message","note":"提交目前使用者訊息｜初始化本回合 agent 上下文","arguments":{"content":"<exact current user message>"}}
```

## 參數

- `content`：必填，必須是目前 user turn 的**完整且精確的使用者訊息**。

## 注意

- 每個 user turn 必須在最開始呼叫一次。
- 後續相同 turn 的其他工具呼叫應重用同一個 `turnId`。
- 不要用此工具傳送 progress、反思、發現或一般 commentary；這些應使用 `agent_progress`。
- 呼叫後應檢查回傳的 `toolRecovery` directive；若需要的 ChatCMD schema 尚未可見，應依 host connector 的 schema discovery 機制載入，而不是直接判定工具不存在。
```