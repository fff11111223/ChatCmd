# agent_plan_question

向使用者提出需要二選一的規劃或執行決策問題。

## 呼叫

```json
{"id":"call_XXX","tool":"agent_plan_question","note":"確認執行方案｜讓使用者在兩個明確選項中決策","arguments":{"question":"要先完成程式碼修改，還是先執行測試？","options":["先修改程式碼","先執行測試"],"questionKind":"clarification"}}
```

## 參數

- `question`：必填，要向使用者提出的問題。
- `options`：必填，**恰好兩個且必須互不相同**的選項。
- `questionKind`：選填，問題類型；預設為 `clarification`。需要取得執行同意時可使用 `executionConsent`。

## 注意

- `options` 必須剛好包含兩個不同選項，不能提供第三個選項。
- 一般規劃澄清使用 `clarification`；需要取得執行同意時使用 `executionConsent`。
- 不要用此工具回報進度或直接完成工作。
