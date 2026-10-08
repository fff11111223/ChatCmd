# fs_replace_text

## 用途

`fs_replace_text` 用於在指定檔案中精確取代一段既有文字。

這是檔案修改工具。使用前應先讀取原始內容，確認要被取代的文字，再進行修改。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"fs_replace_text","arguments":{"path":"D:\\proj\\config.txt","oldText":"old value","newText":"new value"}}
```

上面的呼叫會在 `D:\\proj\\config.txt` 中，將完全符合 `old value` 的文字取代為 `new value`。

## 參數

### `path`

要修改的檔案路徑，必要參數。

### `oldText`

要被取代的原始文字，必要參數。應直接使用 `fs_read_text` 讀取到的內容，不要憑記憶重建。

### `newText`

要寫入的新文字，必要參數。只應包含預期修改後的內容。

## 精確取代規則

`fs_replace_text` 是精確文字取代，不是模糊搜尋工具。

例如原檔案包含：

```text
port=8080
host=localhost
```

可以使用：

```chatcmd_tool_call
{"id":"call_002","tool":"fs_replace_text","arguments":{"path":"D:\\proj\\config.txt","oldText":"port=8080","newText":"port=8081"}}
```

不要把部分文字、猜測內容或正規表示式當成 `oldText`，除非工具本身的實際定義明確支援該行為。

## 建議操作流程

1. 使用 `fs_read_text` 讀取目標檔案。
2. 從讀取結果取得需要修改的完整原文。
3. 呼叫 `fs_replace_text`，指定 `path`、`oldText`、`newText`。
4. 檢查工具回傳結果。
5. 再次使用 `fs_read_text` 或 `git_diff` 驗證修改結果。

## 與其他工具的差異

- `fs_read_text`：唯讀，不修改檔案。
- `fs_replace_text`：精確取代既有文字。
- `fs_apply_edits`：適合一次進行多處結構化修改。
- `fs_write_text`：建立檔案或整份覆寫內容。

## 錯誤處理

若缺少必要參數，工具會回傳 `invalid_arguments`。例如缺少 `path` 時，已確認會回傳：

```text
[invalid_arguments] missing field `path`
```

依核心規則補正參數後最多重試一次。

若遭遇 `policy_denied` 或 `path_outside_allowed_scope`，立即停止，不要改用其他工具繞過限制。

## 注意事項

- 修改前應先讀取原文。
- `oldText` 應來自實際讀取結果，而不是自行猜測。
- 不要把 `fs_replace_text` 當成模糊搜尋或正規表示式取代工具。
- 修改完成後應驗證結果，確認沒有產生非預期變更。
