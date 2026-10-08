# device_get

## 用途

取得指定執行裝置的詳細基本資訊。與 `device_list` 不同，`device_get` 需要指定一個 `deviceId`。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"device_get","arguments":{"deviceId":"0c83182a-03e7-44eb-b51c-9d23c650cab3"}}
```

## 參數

- `deviceId`：必填，指定要查詢的裝置識別碼。

`deviceId` 應使用 `device_list` 實際回傳的識別碼，不要自行猜測。

## 結果重點

成功結果可包含：

- `deviceId`：裝置識別碼。
- `machineId`：機器識別資訊。
- `name`：裝置名稱。
- `online`：目前是否在線。
- `platform`：作業系統平台。
- `osVersion`：作業系統版本。
- `architecture`：CPU 架構。
- `appVersion`：裝置端應用程式版本。

## 常見錯誤

- 缺少 `deviceId` 會得到 `invalid_arguments`。
- 指定不存在或無效的 `deviceId` 時，不應自行改猜其他識別碼；應先用 `device_list` 確認目前可用裝置。

## 使用建議

- 先用 `device_list` 取得裝置清單，再用其中的 `deviceId` 查詢詳細資訊。
- `device_get` 是唯讀查詢，不會修改裝置狀態。

## 已驗證

實際測試 `{}` 會回報缺少 `deviceId`；使用 `device_list` 實際取得的 `deviceId` 可成功查詢，結果包含裝置基本資訊欄位。
