# fs_delete

## 用途

`fs_delete` 用來刪除允許工作區範圍內的檔案或資料夾。

工具預設使用 quarantine 模式；永久刪除必須明確指定對應的刪除模式，不應把一般刪除直接視為永久刪除。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"fs_delete","arguments":{"path":"D:\\proj\\tmp\\old.txt"}}
```

## 預覽刪除

若只需要確認操作而不實際刪除，可使用 `dryRun`：

```chatcmd_tool_call
{"id":"call_002","tool":"fs_delete","arguments":{"path":"D:\\proj\\tmp\\old.txt","dryRun":true}}
```

## 刪除資料夾

刪除資料夾時，可透過 `recursive` 指定是否遞迴處理內容：

```chatcmd_tool_call
{"id":"call_003","tool":"fs_delete","arguments":{"path":"D:\\proj\\tmp\\old-folder","recursive":true}}
```

## 參數

- `path`：要刪除的檔案或資料夾路徑，必填。
- `recursive`：是否遞迴處理資料夾內容，選填。
- `mode`：刪除模式，選填；工具預設為 quarantine 模式，永久刪除必須明確指定。
- `expectedVersion`：目標項目的預期版本，選填，用於避免目標在操作前已被其他操作修改。
- `dryRun`：是否只預覽、不實際刪除，選填。
- `budget`：刪除操作的資源預算，選填。

## 使用方式

1. 先確認 `path` 指向正確的檔案或資料夾。
2. 不確定操作結果時，先使用 `dryRun: true` 預覽。
3. 刪除資料夾時，只有在確定需要處理其內容時才使用 `recursive: true`。
4. 一般刪除預設進入 quarantine；若真的需要永久刪除，必須明確指定刪除模式。
5. 如需要版本一致性保護，可先取得目標版本，再傳入 `expectedVersion`。

## 注意

`fs_delete` 是破壞性操作。遇到 `policy_denied` 或 `path_outside_allowed_scope` 時，應停止，不要改用其他工具繞過限制。
