# task_artifact_list

列出目前 task 已註冊的 artifacts。

## 呼叫

```json
{"id":"call_XXX","tool":"task_artifact_list","note":"列出 task artifacts｜確認目前工作產物","arguments":{}}
```

## 參數

此工具沒有 tool-specific arguments。使用目前 ChatCMD turn 的 taskId correlation。

## 注意

- 不需要在 `arguments` 中自行提供 taskId。
- 這是唯讀工具。
