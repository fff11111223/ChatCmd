# fs_write_raw

## 用途

`fs_write_raw` 用於將原始內容寫入檔案。它與 `fs_write_text` 不同：一般文字檔優先使用 `fs_write_text`，`fs_write_raw` 適合需要以原始位元組內容寫入的情境。

## 基本呼叫：base64

已實際驗證的必要參數為 `path`，內容來源必須在 `base64` 與 `contentRef` 之間二選一。

`base64` 是要寫入的原始位元組內容，以 Base64 表示。

```chatcmd_tool_call
{"id":"call_001","tool":"fs_write_raw","arguments":{"path":"D:\\\\proj\\\\data.bin","base64":"AAECAwQ="}}
```

上述範例會將 Base64 解碼後的原始位元組寫入 `data.bin`。

## 參數

### path

要建立或覆寫的檔案路徑。

### base64

原始內容的 Base64 編碼。與 `contentRef` 必須二選一，不可同時提供，也不能兩者都省略。

```chatcmd_tool_call
{"id":"call_002","tool":"fs_write_raw","arguments":{"path":"D:\\\\proj\\\\image.dat","base64":"AAECAwQFBgcICQ=="}}
```

### contentRef

大型或已透過 Blob 流程準備好的原始內容引用。它與 `base64` 二選一；內容很大時不應把整份資料直接塞進 `base64`。

```chatcmd_tool_call
{"id":"call_003","tool":"fs_write_raw","arguments":{"path":"D:\\\\proj\\\\artifact.bin","contentRef":"<由 Blob 工具取得的 contentRef>"}}
```

## 使用建議

- 一般文字檔優先使用 `fs_write_text`。
- 需要保留原始位元組內容時才使用 `fs_write_raw`。
- `base64` 與 `contentRef` 必須二選一。
- 使用 `base64` 時，內容必須是合法的 Base64。
- 寫入前確認 `path` 位於允許的工作範圍內。
- 寫入完成後，依內容類型使用適當的讀取或狀態工具確認結果。
- 若遭遇 `policy_denied` 或 `path_outside_allowed_scope`，依核心規則停止，不要改用其他工具繞過限制。

## 已確認的參數錯誤

- 缺少 `path`：`invalid_arguments`。
- `path` 存在但 `base64`、`contentRef` 都沒有：`invalidContentSource`。
- `base64` 與 `contentRef` 同時提供時，違反「恰好一個內容來源」規則。
