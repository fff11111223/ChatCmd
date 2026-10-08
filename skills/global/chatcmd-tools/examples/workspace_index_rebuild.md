# workspace_index_rebuild

## 用途

`workspace_index_rebuild` 用於重建指定工作區的搜尋索引。

這不是一般的唯讀查詢；只有在搜尋索引狀態需要重建、且確實有此必要時才使用。執行前可先用 `workspace_index_status` 查詢狀態。

## 基本呼叫

`path` 是工作區路徑。

```chatcmd_tool_call
{"id":"call_001","tool":"workspace_index_rebuild","arguments":{"path":"D:\\proj"}}
```

## 參數

- `path`：要重建搜尋索引的工作區路徑。

目前已從核心工具規則確認 `workspace_index_rebuild` 與 `workspace_index_status` 都以工作區索引為對象；實際使用時以工具目錄的參數定義與實際結果為準。

## 建議流程

先確認索引狀態：

```chatcmd_tool_call
{"id":"call_002","tool":"workspace_index_status","arguments":{"path":"D:\\proj"}}
```

只有確認確實需要重建時，再執行 `workspace_index_rebuild`。

## 使用時機

- 搜尋結果明顯過時，且索引狀態顯示需要更新時。
- 工作區內容已發生大量變更，現有搜尋索引無法正確反映目前內容時。

## 注意事項

- 不要把索引重建當成一般搜尋前的固定步驟。
- `path` 應位於目前允許的工作區範圍內。
- 這是可能耗費時間的操作，沒有明確理由時不要任意執行。
- 若遭遇 `policy_denied` 或 `path_outside_allowed_scope`，依核心規則停止，不要改用其他工具繞過限制。
- 工具的實際回傳欄位應以當次結果為準，不要自行假設未驗證的狀態欄位。
