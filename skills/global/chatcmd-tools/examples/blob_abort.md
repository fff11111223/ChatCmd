# blob_abort

中止尚未完成的 Blob 上傳工作階段。與 `blob_seal` 不同，`blob_abort` 不會完成 Blob，而是將狀態設為 `aborted`。

## 基本用法

```json
{"id":"call_001","tool":"blob_abort","arguments":{"uploadId":"<upload-id>"}}
```

## 參數

- `uploadId`：要中止的 Blob upload ID，必填。

## 成功結果

成功時會回傳 Blob 的狀態資訊，例如：

```json
{
  "contentRef": "blob:v1:...",
  "expectedSizeBytes": null,
  "expiresAtMs": 1791339181178,
  "finalizer": "agent_turn_complete",
  "nextOffset": 0,
  "purpose": "artifact",
  "receivedSizeBytes": 0,
  "requiresFinalization": true,
  "sessionId": "...",
  "sha256": null,
  "state": "aborted",
  "taskId": "...",
  "turnId": "...",
  "uploadId": "...",
  "usage": {
    "elapsedMs": 16,
    "outputBytes": 0
  }
}
```

實際結果中 `state` 會變成 `aborted`；未上傳資料時 `receivedSizeBytes` 與 `nextOffset` 可為 `0`，`sha256` 可為 `null`。

## 使用建議

- 只能使用實際存在且屬於目前工作階段的 `uploadId`。
- 若只想取消尚未完成的上傳，使用 `blob_abort`；不要用 `blob_seal` 代替。
- `blob_abort` 不需要 `finalSizeBytes` 或 `sha256`。
- 假的或不存在的 `uploadId` 會得到 `blobNotFound`，因此不要猜測 upload ID。

## 已驗證錯誤

缺少 `uploadId`：

```json
{"id":"call_001","tool":"blob_abort","arguments":{}}
```

會得到 `missing uploadId`。

不存在的 upload ID：

```json
{"id":"call_002","tool":"blob_abort","arguments":{"uploadId":"invalid-probe-upload-id"}}
```

會得到 `blobNotFound`。
