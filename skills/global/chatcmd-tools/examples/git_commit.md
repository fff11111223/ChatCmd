# git_commit

## 用途

`git_commit` 用來建立 Git commit，也支援先產生 side-effect-free preview，再以 `expectedPreview` 綁定預覽結果執行 commit。

工具要求明確指定 commit scope：使用非空的 `paths`，或指定 `all: true`；兩者互斥。

## 預覽 commit

建議先使用 `previewOnly: true`，確認 repository 狀態與預計提交範圍：

```chatcmd_tool_call
{"id":"call_001","tool":"git_commit","arguments":{"message":"Update documentation","paths":["README.md"],"previewOnly":true}}
```

此操作不建立 commit；成功後取得的 preview 可在後續真正 commit 時放入 `expectedPreview`，讓工具在 commit 前重新檢查 repository 狀態。

## 路徑範圍 commit

指定一個或多個明確路徑：

```chatcmd_tool_call
{"id":"call_002","tool":"git_commit","arguments":{"message":"Update documentation","paths":["README.md","docs\\guide.md"]}}
```

## 指定工作目錄

可使用 `cwd` 指定 Git repository 的工作目錄：

```chatcmd_tool_call
{"id":"call_003","tool":"git_commit","arguments":{"cwd":"D:\\proj","message":"Update documentation","paths":["README.md"],"previewOnly":true}}
```

## 全部變更

若要提交完整 worktree，可使用 `all: true`：

```chatcmd_tool_call
{"id":"call_004","tool":"git_commit","arguments":{"message":"Sync project changes","all":true,"previewOnly":true}}
```

`all` 與 `paths` 互斥；`all: true` 時不可同時提供 `paths`。

## 參數

- `cwd`：Git repository 的工作目錄，選填；亦可使用別名 `path`。
- `message`：commit message，必填。
- `paths`：明確的 commit 路徑清單；使用 `all: false` 時必須是非空陣列。
- `all`：是否使用完整 worktree scope，選填，預設為 `false`，與 `paths` 互斥。
- `previewOnly`：是否只產生預覽而不建立 commit，選填。
- `expectedPreview`：先前 `previewOnly` 取得的預覽結果，選填；提供時會在 commit 前重新驗證 repository 狀態。
- 其他 Git 執行選項：由工具的 Git run options 提供，依實際需求使用。

## 建議流程

1. 確認 Git repository 與要提交的 scope。
2. 使用 `previewOnly: true` 產生預覽。
3. 檢查 preview 是否符合預期。
4. 真正 commit 時提供相同 scope，並可將前一步取得的 preview 傳入 `expectedPreview`。
5. 若 repository 狀態在預覽後發生變化，工具會拒絕過期 preview，而不是直接提交不同內容。

## 注意

`git_commit` 會產生實際 Git commit，是具副作用的操作。正式執行前應優先使用 `previewOnly: true`。

使用 `all: true` 時，工具會處理完整 worktree，且對既有 Git index 有額外保護；若 staging 或 commit 在 HEAD 尚未變更前失敗，runtime 會嘗試恢復原本的 index。
