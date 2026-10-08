# device_list

## 用途

列出目前可用的執行裝置，適合在需要確認裝置是否在線或取得裝置基本資訊時使用。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"device_list","arguments":{}}
```

`device_list` 不需要參數。

## 結果重點

成功結果會回傳裝置清單，每個裝置可能包含：

- `deviceId`：裝置識別碼。
- `machineId`：機器識別資訊。
- `name`：裝置名稱。
- `online`：目前是否在線。
- `platform`：作業系統平台。
- `osVersion`：作業系統版本。
- `architecture`：CPU 架構。
- `appVersion`：裝置端應用程式版本。

## 使用建議

- 需要確認有哪些執行裝置時使用 `device_list`。
- `online` 可用來判斷裝置目前是否在線。
- 若需要查詢特定裝置的詳細資訊，再使用 `device_get`。
- 不要根據裝置名稱或識別碼自行推測不存在的裝置。

## 已驗證

本工具已實際呼叫成功，使用空的 `arguments` 即可取得裝置清單。實際結果包含 `deviceId`、`machineId`、`name`、`online`、`platform`、`osVersion`、`architecture`、`appVersion` 等欄位。
