# fs_create_directory

建立資料夾。適合在需要新的工作目錄、輸出目錄或其他受允許路徑時使用。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"fs_create_directory","arguments":{"path":"D:\\proj\\output"}}
```

### 參數

- `path`：要建立的資料夾路徑，必填。

## 使用方式

- 只建立資料夾，不負責建立或修改其中的檔案。
- 路徑必須位於目前工具允許的範圍內。
- 不確定目標是否已存在時，先用 `fs_stat` 或 `fs_list` 確認。
- 建立已存在的資料夾不會當作成功的 no-op；實測會回傳 `io_error`，Windows `os error 183`（當檔案已存在時，無法建立該檔案）。
- 若收到 `path_outside_allowed_scope` 或 `policy_denied`，依核心規則停止，不要改用其他工具繞過限制。

## 例：建立 examples 子目錄

```chatcmd_tool_call
{"id":"call_002","tool":"fs_create_directory","arguments":{"path":"D:\\proj\\skills\\global\\my-skill\\examples"}}
```

## 已驗證的錯誤

缺少 `path`：

```text
[invalid_arguments] missing field path
```

目標資料夾已存在：

```text
[io_error] 當檔案已存在時，無法建立該檔案。 (os error 183)
```
