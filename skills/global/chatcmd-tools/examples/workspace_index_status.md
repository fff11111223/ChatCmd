# workspace_index_status

## 用途

`workspace_index_status` 用於查詢指定工作區路徑的搜尋索引狀態。它是唯讀工具，不會重建或修改索引。

## 基本呼叫

`path` 是必要參數，指定要查詢索引狀態的工作區路徑。

```chatcmd_tool_call
{"id":"call_001","tool":"workspace_index_status","arguments":{"path":"D:\\proj"}}
```

## 參數

- `path`：要查詢索引狀態的工作區路徑，必要參數。

目前已實測確認，缺少 `path` 會回傳 `invalid_arguments`。

## 使用時機

- 需要確認某個工作區的搜尋索引是否存在或是否需要更新時使用。
- 搜尋結果明顯過時時，可先查詢索引狀態，再決定是否需要使用 `workspace_index_rebuild`。
- 它只查詢索引狀態，不應用來取代一般檔案內容搜尋。

## 與 workspace_index_rebuild 的差異

- `workspace_index_status`：查詢目前索引狀態。
- `workspace_index_rebuild`：重建搜尋索引，屬於會產生修改／耗時操作的工具，只有在確實需要時才使用。

## 注意事項

- `path` 應位於目前允許的工作區範圍內。
- 不要因為索引狀態未知就直接假設索引已損壞；應以工具實際回傳結果判斷。
- 若遭遇 `policy_denied` 或 `path_outside_allowed_scope`，依核心規則停止，不要改用其他工具繞過限制。
