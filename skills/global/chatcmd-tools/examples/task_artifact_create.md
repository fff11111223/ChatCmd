# task_artifact_create

將已封存的 blob artifact `contentRef` 匯入目前 task 授權的 workspace-relative 路徑，並登錄為目前 task 的 artifact。

## 呼叫

```json
{"id":"call_XXX","tool":"task_artifact_create","note":"匯入 sealed artifact｜將已封存產物登錄到 task workspace","arguments":{"content_ref":"<sealed-contentRef>","relative_path":"artifacts/output.bin","media_type":"application/octet-stream"}}
```

## 參數

- `content_ref`：必填，已 sealed 的 artifact content reference。
- `relative_path`：必填，授權 workspace 內的相對路徑。
- `media_type`：選填，產物的 MIME type。

## 注意

- `content_ref` 必須是已封存的 artifact；不能直接使用未完成的 blob upload。
- `relative_path` 應使用 workspace-relative 路徑，不要自行提供絕對路徑。
- 這是會建立 task artifact 的修改工具；只有在確實需要匯入產物時才執行。
