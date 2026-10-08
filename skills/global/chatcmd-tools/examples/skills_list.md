# skills_list

## 用途

列出目前可用的 Skill，以及各 Skill 是否具有 core、resident、examples 等內容。適合在需要確認有哪些 Skill 可用時使用。

## 基本呼叫

```chatcmd_tool_call
{"id":"call_001","tool":"skills_list","arguments":{}}
```

`skills_list` 不需要參數。

## 結果重點

成功結果會包含 Skill 清單，每個項目可能包含：

- `id`：Skill 識別名稱。
- `name`：Skill 名稱。
- `title`：Skill 顯示名稱。
- `description`：Skill 說明。
- `source`：Skill 來源路徑。
- `hasCore`：是否有 core 提示。
- `hasResident`：是否有 resident 提示。
- `examples`：目前可用的 example 工具名稱清單。

## 使用建議

- 需要確認 Skill 是否存在時，先使用 `skills_list`。
- 確認特定 Skill 後，再使用 `skill_read` 讀取其 resident、core 或特定工具規則。
- 不要根據不存在於結果中的 Skill 或 example 名稱自行猜測。

## 已驗證

本工具已實際呼叫成功，使用空的 `arguments` 即可取得 Skill 清單。
