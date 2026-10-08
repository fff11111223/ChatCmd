# task_get

取得目前 task 的狀態。

## 呼叫

```json
{"id":"call_XXX","tool":"task_get","note":"取得目前 task 狀態｜確認目前工作上下文","arguments":{}}
```

## 參數

此工具沒有 tool-specific arguments。使用目前 ChatCMD turn 的 taskId correlation。

## 注意

- 不需要在 `arguments` 中自行提供 taskId。
- 這是唯讀工具。
