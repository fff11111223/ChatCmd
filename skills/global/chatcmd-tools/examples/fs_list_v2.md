# fs_list_v2

## 用途
列出指定資料夾內容。這是 `_v2` 版本的目錄列舉工具。

## 參數

```json
{
  "path": "D:\\frank\\gemini\\ChatCmd\\skills\\global\\chatcmd-tools\\examples"
}
```

- `path`：要列出的資料夾路徑。

## 成功範例

```text
fs_list_v2({"path":"D:\\frank\\gemini\\ChatCmd\\skills\\global\\chatcmd-tools\\examples"})
```

成功結果包含：

```json
{
  "data": {
    "directoryVersion": "sha256:...",
    "items": [
      {
        "name": "fs_list_v2.md",
        "path": "\\\\?\\D:\\frank\\gemini\\ChatCmd\\skills\\global\\chatcmd-tools\\examples\\fs_list_v2.md"
      }
    ],
    "sort": "filesystem"
  },
  "page": {
    "hasMore": false
  },
  "schemaVersion": 1
}
```

## 注意

- `items` 是目錄項目清單，每項包含 `name` 與 `path`。
- `directoryVersion` 可用來識別該目錄版本。
- `page.hasMore` 表示是否還有後續頁面。
- 實際結果還可能包含 `finalizer`、`sessionId`、`taskId`、`turnId`、`usage` 等執行資訊；這些不是呼叫參數。
- 不要把 `fs_list` 的參數格式套用到 `fs_list_v2`；應依 `_v2` 工具的實際定義呼叫。
