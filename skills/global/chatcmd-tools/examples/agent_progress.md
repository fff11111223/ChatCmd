# agent_progress

向 agent runtime 回報目前工作進度。

## 呼叫

```json
{"id":"call_XXX","tool":"agent_progress","note":"回報目前進度｜告知工作已完成的階段與下一步","arguments":{"message":"已完成目前階段的檢查，下一步處理剩餘項目","suggestedTitle":"檢查進度"}}
```

## 參數

- `message`：必填，進度訊息。
- `suggestedTitle`：選填，給進度項目的建議標題。

## 注意

- 用於回報工作進度，不是最終完成回覆。
- `suggestedTitle` 可省略；只有需要提供簡短進度標題時才使用。
- 不要把尚未驗證的結果描述成已完成。
