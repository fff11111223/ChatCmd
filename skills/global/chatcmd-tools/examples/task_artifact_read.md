# task_artifact_read

讀取指定 task artifact 的一段資料。大型 artifact 可透過 `offset` 與 `maxBytes` 分段讀取。

## 呼叫

```json
{"id":"call_XXX","tool":"task_artifact_read","note":"讀取 task artifact 範圍｜取得指定產物的第一段內容","arguments":{"artifact_id":"<artifactId>","offset":0,"max_bytes":65536}}
```

## 參數

- `artifact_id`：必填，要讀取的 task artifact ID。
- `offset`：選填，讀取起始 byte offset；未提供時由工具採用預設起點。
- `max_bytes`：選填，本次最多讀取的 bytes。

## 分段讀取

若結果包含 `nextOffset` 且 `hasMore` 為 `true`，下一次呼叫可將 `nextOffset` 作為新的 `offset`，繼續讀取。

## 注意

- 這是唯讀工具。
- 不要自行猜測 `artifact_id`；應先從 `task_artifact_list` 取得有效 artifact。
