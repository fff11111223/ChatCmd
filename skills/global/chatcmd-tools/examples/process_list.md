# process_list

## 用途

列出 execution host 上目前正在執行的本機程序。這是唯讀查詢，可搭配 `process_inspect` 取得特定程序的額外資訊。

## 基本呼叫

```chatcmd_tool_call
{
  "id": "call_001",
  "tool": "process_list",
  "arguments": {}
}
```

## 參數

不需要提供工具專用參數，`{}` 即可執行。

## 使用方式

1. 使用 `process_list` 取得目前程序清單。
2. 從結果找出需要進一步查詢的程序 ID。
3. 將實際的程序 ID 傳給 `process_inspect`，取得單一程序資訊。
4. 若目的是終止程序，不應把 `process_list` 當成終止工具；應依需求使用具備相應權限的工具。

## 與 process_inspect 的差異

- `process_list`：列出多個本機程序，不需要 `processId`。
- `process_inspect`：查詢單一程序，必須提供 `processId`。

## 常見錯誤

- 不要自行在 `process_list` 中加入 `processId`、PID 或其他未定義欄位。
- 不要假設程序一定存在；程序清單反映的是執行當下的 execution host 狀態。

## 已驗證

- 工具註冊型別為 `NoArgs`。
- `{}` 可直接呼叫。
- `docs/mcp_method.md` 將其描述為列出 execution host 上正在執行的本機程序。
- `process_inspect` 範例已驗證可使用 `process_list` 取得的實際程序 ID 作為後續查詢依據。
