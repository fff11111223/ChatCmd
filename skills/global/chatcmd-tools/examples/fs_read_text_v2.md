# fs_read_text_v2 範例

## 什麼時候用

讀取文字檔內容，使用結構化的 `range` 指定讀取範圍。

## 參數

- `path`：要讀取的文字檔路徑。
- `range`：讀取範圍。
  - `unit`：範圍單位；本範例確認 `line`。
  - `start`：起始行號。
  - `limit`：最多讀取的行數。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_XXX","tool":"fs_read_text_v2","note":"讀取文字範圍｜讀取指定檔案前 5 行","arguments":{"path":"D:\\frank\\gemini\\ChatCmd\\skills\\global\\chatcmd-tools\\examples\\fs_read_text.md","range":{"unit":"line","start":1,"limit":5}}}
```

## 實際成功結果

```json
{
  "bom": false,
  "bytesRead": 109,
  "content": "# fs_read_text 範例\r\n\r\n## 什麼時候用\r\n\r\n讀一個文字檔的內容，或其中一段行數範圍。\r\n",
  "encoding": "utf-8",
  "lineEnding": "crlf",
  "lineEndingDetection": "sampled",
  "nextByteOffset": 109,
  "nextStartLine": 6,
  "path": "\\\\?\\D:\\frank\\gemini\\ChatCmd\\skills\\global\\chatcmd-tools\\examples\\fs_read_text.md",
  "range": {
    "endByte": 109,
    "endLine": 5,
    "startByte": null,
    "startLine": 1
  },
  "sizeBytes": 1684,
  "totalLines": null,
  "totalLinesKnown": false,
  "truncated": true,
  "truncationReason": "rangeLimit",
  "versionToken": "v1-ce74c776eb7e9f3e"
}
```

## 注意

- `range` 不使用舊版的 `startLine` / `lineCount` 欄位。
- `unit:"line"` 時，使用 `start` 與 `limit` 指定行範圍。
- 當實際內容超過 `limit` 時，結果會以 `truncated:true` 並以 `truncationReason:"rangeLimit"` 表示。
