# blob_begin

## 用途

`blob_begin` 用於開始一次大型內容的分段傳送。它會建立 Blob 上傳工作，後續可搭配 `blob_write_chunk`、`blob_status`、`blob_seal` 或 `blob_abort` 完成傳送流程。

內容不大時，優先使用 `fs_write_text`；只有需要大型內容分段傳送時才使用 Blob 工具。

## 基本呼叫

`purpose` 是必要參數，且必須是工具支援的用途之一。

```chatcmd_tool_call
{"id":"call_001","tool":"blob_begin","arguments":{"purpose":"artifact"}}
```

## 參數

### purpose

`purpose` 必填，用來指定 Blob 的用途。已確認的合法值為：

- `fsWriteText`
- `fsWriteRaw`
- `fsApplyEdits`
- `artifact`

例如，若用途是一般 Artifact 傳送，可以使用：

```chatcmd_tool_call
{"id":"call_002","tool":"blob_begin","arguments":{"purpose":"artifact"}}
```

不要自行建立其他 `purpose` 值；未知值會得到 `invalidBlobPurpose`。

## 回傳結果

成功結果會提供後續 Blob 操作所需的資訊。已實際確認的欄位包括：

- `contentRef`：Blob 內容參照，例如 `blob:v1:...`。
- `uploadId`：這次上傳工作的識別碼。
- `chunkSizeBytes`：建議的分段大小；實際測試結果為 `1048576` Bytes。
- `maxSizeBytes`：此次 Blob 的最大大小；實際測試結果為 `1073741824` Bytes。
- `expiresAtMs`：Blob 上傳工作的到期時間（Unix epoch milliseconds）。
- `requiresFinalization`：表示是否需要後續 finalization。
- `usage`：此次呼叫的使用量資訊，例如 `elapsedMs`、`outputBytes`。

## 使用流程

典型流程為：

1. `blob_begin` 建立 Blob。
2. 使用回傳的 Blob 識別資訊進行 `blob_write_chunk`。
3. 需要確認狀態時使用 `blob_status`。
4. 所有內容傳送完成後使用 `blob_seal` 完成 Blob。
5. 若傳送中止或不再需要，使用 `blob_abort`。

具體的後續參數應以各工具自己的工具目錄定義為準，不要從 `blob_begin` 的回傳欄位自行推測其他工具的參數名稱。

## 常見錯誤

- 缺少 `purpose`：會得到 `invalid_arguments`，並提示 `missing field \`purpose\``。
- `purpose` 不在支援清單中：會得到 `invalidBlobPurpose`。
- `purpose` 只能使用 `fsWriteText`、`fsWriteRaw`、`fsApplyEdits` 或 `artifact`。

若遭遇 `policy_denied` 或 `path_outside_allowed_scope`，依核心規則停止，不要改用其他工具繞過限制。

## 注意事項

- `blob_begin` 會建立實際的 Blob 上傳工作，不是單純的唯讀查詢；不要為了猜測參數而反覆建立 Blob。
- 大型內容才使用 Blob；小型文字內容優先使用 `fs_write_text`。
- `chunkSizeBytes` 與 `maxSizeBytes` 應以當次 `blob_begin` 回傳值為準，不要寫死成固定限制。
