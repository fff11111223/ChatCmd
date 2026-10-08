# process_inspect

## 用途

查詢指定程序的基本資訊。與 `process_list` 不同，`process_inspect` 只查詢單一程序。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"process_inspect","arguments":{"processId":35404}}
```

## 參數

- `processId`：必填，指定要查詢的程序 ID。

## 成功結果

實際查詢 `explorer.exe` 時，結果包含：

```json
{
  "name": "explorer.exe",
  "processId": 35404,
  "details": "\"explorer.exe\",\"35404\",\"Console\",\"5\",\"245,464 K\""
}
```

`details` 是程序的額外資訊字串，具體內容依程序而異。

## 常見錯誤

- 缺少 `processId` 會得到 `invalid_arguments`。
- 不應自行猜測程序 ID；需要 ID 時可先從 `process_list` 取得。

## 使用建議

- 只需要單一程序資訊時使用 `process_inspect`。
- 不涉及終止或修改程序。

## 已驗證

實際測試 `{}` 會回報缺少 `processId`；使用實際存在的 `explorer.exe` PID 35404 可成功取得程序資訊。
