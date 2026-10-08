# fs_copy

## 用途

`fs_copy` 用來複製檔案或資料夾。

與 `fs_move` 不同，`fs_copy` 保留來源，不會把來源搬走。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"fs_copy","arguments":{"source":"D:\\proj\\docs\\README.md","destination":"D:\\backup\\README.md"}}
```

## 參數

- `source`：來源檔案或資料夾路徑，必填。
- `destination`：複製後的目的地路徑，必填。

## 使用方式

1. 先確認 `source` 存在且位於允許的工作區範圍。
2. 指定 `destination` 作為複製目標。
3. 複製完成後，如需要確認結果，再使用 `fs_stat` 或 `fs_list` 檢查目的地。
4. 不要把 `fs_copy` 當成改名或搬移工具；需要搬移或改名時使用 `fs_move`，但該工具具有破壞性。

## 已確認的參數錯誤

只有 `source` 時，工具回報：

```text
[invalid_arguments] missing field `destination`
```

空參數時，工具先回報：

```text
[invalid_arguments] missing field `source`
```

因此目前已確認 `source` 與 `destination` 都是必要參數。

## 注意

`fs_copy` 的實際目的地是否允許寫入，仍受 ChatCMD 的工具權限與工作區範圍限制；遇到 `policy_denied` 或 `path_outside_allowed_scope` 時，應停止，不要改用其他工具繞過限制。
