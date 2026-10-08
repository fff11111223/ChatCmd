# fs_apply_edits

## 用途

`fs_apply_edits` 用於一次對同一個檔案套用多處結構化修改。與 `fs_replace_text` 的單段精確取代不同，它以 `edits` 陣列描述多個修改位置。

## 基本呼叫

已確認的基本參數包括 `path`、`expectedVersion`、`coordinateSystem` 與 `edits`。`edits` 至少要包含一筆修改。

```chatcmd_tool_call
{"id":"call_001","tool":"fs_apply_edits","arguments":{"path":"D:\\\\proj\\\\README.md","expectedVersion":"<從 fs_stat 取得的 versionToken>","coordinateSystem":"byte","edits":[{"startByte":0,"endByte":10,"text":"new text"}]}}
```

## 參數

### path

要修改的檔案路徑。

### expectedVersion

檔案目前版本的驗證 token。應先透過檔案狀態工具取得，不要自行猜測或產生。

### coordinateSystem

座標系統。已由工具實際驗證的合法值包括：

- `byte`：以 Byte offset 指定修改範圍。
- `lineColumn`：以行／欄座標指定修改範圍。

### edits

修改項目的陣列，至少需要一筆 edit。

在 `byte` 座標系統下，工具已實際驗證每筆 edit 使用：

- `startByte`
- `endByte`
- `text`

```chatcmd_tool_call
{"id":"call_002","tool":"fs_apply_edits","arguments":{"path":"D:\\\\proj\\\\src\\\\main.rs","expectedVersion":"<versionToken>","coordinateSystem":"byte","edits":[{"startByte":120,"endByte":135,"text":"replacement"},{"startByte":240,"endByte":246,"text":"updated"}]}}
```

## 使用建議

- 修改前先取得目前檔案版本，使用實際取得的 `versionToken`。
- 不要使用不存在的 `coordinateSystem` 值；工具已確認 `byte` 與 `lineColumn` 為合法值。
- `edits` 不可為空陣列。
- `byte` 模式的 edit 只使用 `startByte`、`endByte` 與 `text`。
- 若版本驗證失敗，重新取得檔案最新版本後再判斷是否能重試。
- 修改完成後應使用 `git_diff` 或讀取檔案內容確認結果。
- 若遭遇 `policy_denied` 或 `path_outside_allowed_scope`，依核心規則停止，不要改用其他工具繞過限制。

## 已確認的參數錯誤

- 缺少 `path`：`invalid_arguments`。
- 缺少 `expectedVersion`：`invalid_arguments`。
- `coordinateSystem: "line"`：`invalid_arguments`；合法值為 `byte` 或 `lineColumn`。
- 同時缺少 `edits` 與 `contentRef`：`invalidContentSource`；必須二選一。
- `edits: []`：`editsEmpty`。
- edit 缺少 `text`：`invalid_arguments`。
- byte edit 缺少 `startByte`／`endByte`：`invalidEditCoordinates`。
