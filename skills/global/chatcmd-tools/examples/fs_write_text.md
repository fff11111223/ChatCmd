# fs_write_text

## 用途

將文字內容寫入指定的文字檔案。適合建立新文字檔或以完整內容覆寫既有文字檔。

## 呼叫格式

```chatcmd_tool_call
{"id":"call_001","tool":"fs_write_text","arguments":{"path":"D:\\proj\\notes\\example.md","content":"# Example\n\n這是測試內容。"}}
```

## 參數

- `path`：目標檔案路徑。
- `content`：要寫入的完整文字內容。
- `contentRef`：內容參照。`content` 與 `contentRef` 必須二選一，不能同時提供，也不能兩者都省略。

## 使用原則

- `fs_write_text` 是檔案寫入工具，不要用 `command_run` 搭配 PowerShell 或其他 shell 代替。
- 寫入前確認目標路徑位於允許的 workspace 範圍。
- 若目標檔案已存在，應將 `content` 視為要寫入的完整檔案內容，不要假設它會只修改某一段。
- 大型內容優先使用 `contentRef`，避免把大量文字直接塞進工具參數。

## 常見錯誤

### 缺少內容來源

如果沒有提供 `content` 或 `contentRef`，工具會拒絕要求，例如：

```text
[invalidContentSource] provide exactly one of content or contentRef
```

此時補上其中一個內容來源即可。

### 同時提供兩種內容來源

`content` 與 `contentRef` 必須二選一，同時提供會造成參數錯誤。

### policy_denied / path_outside_allowed_scope

若工具回報 `policy_denied` 或 `path_outside_allowed_scope`，立即停止，不要改用 `command_run` 或其他工具繞過限制。

## 與其他檔案工具的區別

- 讀取檔案：`fs_read_text`
- 搜尋檔案內容：`fs_search` / `fs_find`
- 寫入完整文字內容：`fs_write_text`
- 局部文字替換：`fs_replace_text`
- 多處精確編輯：`fs_apply_edits`

## 結果處理

工具成功後，應以工具回傳結果確認寫入是否完成。若需要確認檔案內容，可再使用 `fs_read_text` 讀取，不要僅根據呼叫本身推測檔案一定已正確寫入。