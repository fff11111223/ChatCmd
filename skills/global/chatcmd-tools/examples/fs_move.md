# fs_move

## 用途

`fs_move` 用來在允許的工作區範圍內搬移檔案或資料夾，也可用於改名。

與 `fs_copy` 不同，`fs_move` 成功後會移除來源項目。工具會先完成必要的跨裝置 staging 與驗證，再發布結果並移除來源。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"fs_move","arguments":{"source":"D:\\proj\\docs\\README.md","destination":"D:\\proj\\archive\\README.md"}}
```

## 參數

- `source`：來源檔案或資料夾路徑，必填。
- `destination`：目的地路徑，必填。
- `overwrite`：是否允許覆寫目的地，選填。
- `conflictPolicy`：目的地衝突處理策略，選填。
- `atomicPublish`：是否使用原子發布，選填，預設為 `true`。
- `verify`：搬移後的驗證模式，選填。
- `preserveMetadata`：是否保留檔案 metadata，選填，預設為 `true`。
- `followSymlinks`：是否跟隨 symbolic links，選填。
- `dryRun`：是否只預覽而不實際搬移，選填。
- `expectedSourceVersion`：來源版本預期值，選填，用於避免來源在操作前已被其他操作修改。
- `expectedDestinationVersion`：目的地版本預期值，選填。
- `budget`：搬移操作的資源預算，選填。

## 使用方式

1. 先確認 `source` 存在且位於允許的工作區範圍。
2. 指定 `destination`。
3. 一般搬移可只提供 `source` 與 `destination`。
4. 搬移完成後，如需要確認結果，可使用 `fs_stat` 或 `fs_list` 檢查目的地。
5. 不要把 `fs_move` 當成單純複製工具；成功的搬移會移除來源。

## 注意

`fs_move` 的實際來源與目的地是否允許操作，仍受 ChatCMD 的工具權限與工作區範圍限制。遇到 `policy_denied` 或 `path_outside_allowed_scope` 時，應停止，不要改用其他工具繞過限制。
