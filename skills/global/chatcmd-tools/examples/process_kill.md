# process_kill

## 用途

`process_kill` 用來依指定的 process ID 終止本機 process。

必要參數是 `processId`；可選擇是否連同整個 process tree 一起終止。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"process_kill","arguments":{"processId":12345}}
```

## 終止整個 Process Tree

若需要連同由指定 process 所建立的子 process 一起終止，可指定 `entireTree: true`：

```chatcmd_tool_call
{"id":"call_002","tool":"process_kill","arguments":{"processId":12345,"entireTree":true}}
```

## 參數

- `processId`：要終止的本機 process ID，必填。
- `entireTree`：是否連同整個 process tree 一起終止，選填。

## 使用方式

1. 先透過 `process_list` 或 `process_inspect` 確認正確的 process ID。
2. 確認該 process 確實是需要終止的目標。
3. 一般情況使用 `processId` 即可。
4. 若需要一併終止子 process，再指定 `entireTree: true`。

## 注意

`process_kill` 是具破壞性的 process 終止操作。不要因為無法確認目標 process 就隨意使用 PID。

若工具回報 `policy_denied`，應停止操作，不要改用其他工具繞過權限限制。
