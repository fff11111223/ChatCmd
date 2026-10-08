# agent_subagent_start

建立或重用一個 child agent，並可指定委派限制。

## 呼叫

```json
{"id":"call_XXX","tool":"agent_subagent_start","note":"建立子代理｜委派明確範圍的工作","arguments":{"name":"repo-reviewer","request":"檢查指定範圍的程式碼並回報發現","allowed_files":["src/example.rs"],"allowed_effects":["read"],"dependencies":[],"acceptance":["回報檢查結果"],"project_context_ref":"<projectContextRef>","instructions_version":"<instructionsVersion>"}}
```

## 參數

- `name`：必填，child agent 名稱。
- `request`：必填，委派給 child agent 的工作要求。
- `allowed_files`：選填，限制 child agent 可處理的檔案範圍。
- `allowed_effects`：選填，限制可產生的效果。
- `dependencies`：選填，指定工作依賴。
- `acceptance`：選填，指定完成條件。
- `project_context_ref`：選填，指定專案上下文 reference。
- `instructions_version`：選填，指定 instructions 版本。
- `approval_grant`：選填，只有存在已核准且可涵蓋的 parent safe-read grant 時才提供；不是 child 的工具 allowlist。

## 注意

- `approval_grant` 的 `allowed_tools` 只能使用 server policy 明確允許的工具名稱；不可加入 Git、process 或 `agent_*` lifecycle 工具。
- child agent 回傳的是 bounded report，應檢查 `files`、`symbols`、`changes`、`evidenceRefs`、`blockers` 與 `workOutcome`。
- 不要把 child agent 的報告視為自動驗證結果。
- 這是建立或重用 child agent 的工具；只有確實需要委派工作時才使用。
```