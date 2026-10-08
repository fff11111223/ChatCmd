# fs_find

## 用途

`fs_find` 用於在指定路徑範圍內尋找符合條件的檔案或目錄。它與 `fs_search` 不同：`fs_find` 是以路徑／檔名模式尋找項目，而不是搜尋檔案內容。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"fs_find","arguments":{"path":"D:\\proj\\src","pattern":"routes.rs","patternMode":"literal"}}
```

## 參數

- `path`：要搜尋的根目錄或路徑。
- `pattern`：搜尋模式。
- `patternMode`：模式類型。已確認可使用 `literal` 與 `glob`。

### literal

`literal` 會將 pattern 視為文字進行比對。

```chatcmd_tool_call
{"id":"call_002","tool":"fs_find","arguments":{"path":"D:\\proj\\src","pattern":"chatgpt","patternMode":"literal"}}
```

### glob

`glob` 可使用萬用字元，例如 `*.rs` 尋找 Rust 原始碼檔案。

```chatcmd_tool_call
{"id":"call_003","tool":"fs_find","arguments":{"path":"D:\\proj\\src","pattern":"*.rs","patternMode":"glob"}}
```

## 回傳結果

成功結果的 `data.items` 會列出符合條件的項目。每個項目至少包含：

- `entryType`：項目類型，例如 `file`。
- `path`：符合條件的完整路徑。

`page.hasMore` 表示是否還有後續結果。

## 注意事項

- `path`、`pattern` 是必要參數；缺少任一項會得到 `invalid_arguments`。
- 建議明確指定 `patternMode`，避免使用未指定模式時的 legacy 行為。
- `literal` 適合尋找特定名稱或文字。
- `glob` 適合依副檔名或檔名規則尋找，例如 `*.rs`。
- `fs_find` 找不到結果時仍可能是成功回傳，應檢查 `data.items` 是否為空，而不是把空結果當成工具錯誤。
- 若遭遇 `policy_denied` 或 `path_outside_allowed_scope`，依核心規則停止，不要改用其他工具繞過限制。
