# task_set_execution_mode

設定目前 task 的 execution mode。

## 呼叫

```json
{"id":"call_XXX","tool":"task_set_execution_mode","note":"設定 task execution mode｜依需求切換目前 task 的執行模式","arguments":{"mode":"<mode>"}}
```

## 參數

- `mode`：必填，execution mode 字串。

## 注意

- 本工具目前可確認的 tool-specific schema 只有必填 `mode`；實際允許值應依執行環境提供的 mode 定義，不要自行猜測。
- 這是 task execution mode 的設定工具，會改變 task 狀態；只有使用者明確要求切換 execution mode 時才執行。
