# agent_turn_complete

完成目前 agent turn，提交最終回覆、工作結果、驗證狀態與證據。

## 呼叫

```json
{"id":"call_XXX","tool":"agent_turn_complete","note":"完成目前工作回合｜提交最終結果與驗證證據","arguments":{"content":"已完成指定工作。","workOutcome":"completed","verificationIntent":"notRun","verificationReason":"本回合為文件整理工作，未執行額外測試。","verificationScope":"skills/global/chatcmd-tools/examples","criteria":[{"criterion":"所有指定範例已建立","evidenceRefs":["<executionId>"]}],"evidenceRefs":["<executionId>"],"blockers":[],"limitations":[]}}
```

## 參數

- `content`：必填，最終提供給使用者的回覆內容。
- `suggestedTitle`：選填，最終回覆的建議標題。
- `workOutcome`：選填，工作結果：`completed`、`partial` 或 `blocked`。
- `verificationIntent`：選填，驗證意圖；目前可使用 `notRun` 或 `notApplicable`。
- `verificationReason`：選填，未執行或不適用驗證時的原因。
- `verificationScope`：選填，本次驗證涵蓋的範圍。
- `criteria`：選填，完成條件清單；每個條件可附 `evidenceRefs`。
- `evidenceRefs`：選填，server-owned execution evidence reference，例如 `command_run` 的 `executionId`。
- `blockers`：選填，目前阻礙工作的項目。
- `limitations`：選填，已知限制。

## 注意

- 這是 agent turn 的最終完成工具，應在其他工具呼叫全部完成後使用。
- 一個 turn 必須恰好完成一次，且應緊接在最後一個工具操作之後。
- `workOutcome` 描述工作是否完成；不要因為沒有執行驗證就誤填成 `blocked`。
- `verificationIntent` 與 `workOutcome` 是不同概念；未測試的程式碼應誠實標示未執行驗證。
- `evidenceRefs` 必須使用實際由 server 提供的 execution evidence，不要自行捏造 execution ID。
- `criteria` 可將各完成條件與對應證據明確關聯。
- 若工作只涉及文件審查或其他不需要驗證的工作，可使用 `verificationIntent:"notApplicable"` 並提供 `verificationReason`。
