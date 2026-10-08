# agent_subagent_wait

等待 child agent 完成，或讀取指定 child agent 的 bounded report 分頁。

## 呼叫

```json
{"id":"call_XXX","tool":"agent_subagent_wait","note":"等待子代理結果｜取得 child agent 的 bounded report","arguments":{"timeout_ms":30000}}
```

## 參數

- `timeout_ms`：選填，等待時間（毫秒）。
- `subagent_id`：選填，指定要讀取報告的 descendant child agent；可用於不等待其他 child 的情況下讀取指定報告頁。
- `report_offset`：選填，從 `report.continuation` 開始計算的 zero-based Unicode character offset。
- `report_version`：選填，報告 continuation 的 immutable final event ID；當 `report_offset` 大於 `0` 時必須提供。

## 分頁讀取

若報告含有 continuation，可用 `report_offset` 從指定 Unicode 字元位置繼續讀取；當 offset 大於 `0` 時，必須同時帶入對應的 `report_version`，避免跨版本讀取報告。

## 注意

- 不提供 `subagent_id` 時，可等待目前工作流程中的 child agent。
- 指定 `subagent_id` 時，工具可針對該 descendant 讀取報告，而不必等待其他 child。
- 不要自行猜測 `subagent_id` 或 `report_version`；應使用 `agent_subagent_start` 或先前 wait 結果提供的實際值。
- `timeout_ms` 控制等待，不代表 child agent 一定會在該時間內完成。
