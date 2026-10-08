# blob_seal

## 用途

`blob_seal` 用於完成一個已分段上傳的 Blob。封存時會驗證最終大小與 SHA-256，成功後 Blob 狀態會變成 `sealed`。

它通常接在 `blob_begin`、`blob_write_chunk` 之後使用。若內容尚未全部寫入，應先依 `blob_write_chunk` 回傳的 `nextOffset` 繼續上傳。

## 基本呼叫

`blob_seal` 至少需要三個參數：`uploadId`、`finalSizeBytes`、`sha256`。

```chatcmd_tool_call
{"id":"call_001","tool":"blob_seal","arguments":{"uploadId":"<uploadId>","finalSizeBytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}}
```

其中 `sha256` 必須是實際上傳內容的 SHA-256，而不是任意字串。

## 參數

### uploadId

`uploadId` 是 `blob_begin` 建立 Blob 時回傳的上傳識別碼。

它必須屬於目前可使用的 Agent／Task／Turn。跨回合或其他工作階段的 Blob 可能會被拒絕為 `blobAccessDenied`。

### finalSizeBytes

`finalSizeBytes` 是最終 Blob 的內容大小，以 Byte 計算。

它必須與實際已寫入的內容大小一致。空 Blob 的值為 `0`。

### sha256

`sha256` 是最終內容的 SHA-256 雜湊值，使用十六進位字串表示。

例如空內容的 SHA-256 為：

```text
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
```

## 實際成功結果

使用空 Blob 實測成功後，結果包含：

```text
{
  "contentRef": "blob:v1:...",
  "expectedSizeBytes": null,
  "expiresAtMs": 1791272125427,
  "finalizer": "agent_turn_complete",
  "nextOffset": 0,
  "purpose": "artifact",
  "receivedSizeBytes": 0,
  "requiresFinalization": true,
  "sessionId": "...",
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "state": "sealed",
  "taskId": "...",
  "turnId": "...",
  "uploadId": "...",
  "usage": {
    "elapsedMs": 24,
    "outputBytes": 0
  }
}
```

最重要的結果欄位包括：

- `state`：成功封存後為 `sealed`。
- `contentRef`：Blob 的內容參照。
- `receivedSizeBytes`：實際已接收的內容大小。
- `sha256`：封存後確認的 SHA-256。
- `uploadId`：此次上傳的識別碼。
- `nextOffset`：目前內容的下一個 Offset；已封存的空 Blob 為 `0`。
- `purpose`：Blob 建立時指定的用途，例如 `artifact`。
- `expiresAtMs`：Blob 的到期時間。
- `requiresFinalization`：是否需要最終化處理。

## 建議流程

一般大型內容的流程為：

1. `blob_begin` 建立 Blob，取得 `uploadId`。
2. 使用 `blob_write_chunk` 依 Offset 分段寫入內容。
3. 確認所有內容都已寫入，並計算完整內容的 SHA-256。
4. 呼叫 `blob_seal`，提供最終大小與 SHA-256。
5. 確認結果中的 `state` 為 `sealed`。

```chatcmd_tool_call
{"id":"call_002","tool":"blob_seal","arguments":{"uploadId":"<uploadId>","finalSizeBytes":<totalBytes>,"sha256":"<sha256>"}}
```

## 常見錯誤

- 缺少 `uploadId`、`finalSizeBytes` 或 `sha256`：會得到 `invalid_arguments`。
- `finalSizeBytes` 與實際接收大小不一致：無法正確完成封存。
- `sha256` 與實際內容不一致：無法通過完整性驗證。
- `uploadId` 不屬於目前可使用的 Agent／Task／Turn：可能得到 `blobAccessDenied`。
- 若遭遇 `policy_denied` 或 `path_outside_allowed_scope`，依核心規則停止，不要改用其他工具繞過限制。

## 注意事項

- `blob_seal` 是 Blob 上傳流程的最終封存步驟，不是一般小型文字檔的寫入工具。
- 小型文字內容應優先使用 `fs_write_text`，不需要為此建立 Blob。
- 封存前應確實取得完整內容的 Byte 大小與 SHA-256。
- 不要把其他回合取得的 `uploadId` 當成目前回合可用的 Blob。
