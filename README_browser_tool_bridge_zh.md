# ChatGPT Browser Tool Bridge 使用說明

> 讓 ChatGPT Web 能夠直接呼叫你本機的 ChatCMD 工具，並把結果回饋給 ChatGPT 繼續執行任務。

---

## 概念說明

### 傳統流程（無 Tool Bridge）

```
使用者 → ChatCMD → ChatGPT Web
                        ↓
              ChatGPT 回應純文字
                        ↓
              ChatCMD 擷取、結束
```

### Tool Bridge 流程

```
使用者 → ChatCMD → ChatGPT Web
                        ↓
         ChatGPT 在回應中輸出 tool_call 區塊
                        ↓
         Extension 擷取 → 呼叫 ChatCMD 本機工具
         （透過 RuntimeHost::call，完整走授權/審核/Timeline）
                        ↓
         Extension 把 tool_result 注入 ChatGPT 對話框 → 送出
                        ↓
         ChatGPT 讀取結果 → 繼續執行 → 最終回應
                        ↓
         ChatCMD 擷取最終回應、完成 Task
```

---

## 環境需求

| 項目 | 需求 |
|------|------|
| ChatCMD | 本文件對應的最新版（已含 `/api/local/chatgpt/bridge/tools/call`） |
| Chrome Extension | 版本 ≥ `0.1.12`（manifest 已加入 `content-chatgpt-tool-bridge.js`） |
| ChatGPT | 已登入 `chatgpt.com` 的瀏覽器 Tab |
| MCP Agent | ChatCMD 內至少一個「已啟用」的 MCP Agent |

---

## 安裝與設定

### 1. 編譯並啟動 ChatCMD

###連前端一起重新 build
cd D:\frank\gemini\ChatCmd\web
npm.cmd run build

cd ..
cargo build --release


```bash
cargo build --release
./target/release/chat-cmd-client
```

ChatCMD 預設監聽 `http://localhost:<port>`。  
確認 Local UI 可以正常連線（瀏覽器開啟 `http://localhost:<port>`）。

### 2. 載入 Chrome Extension

1. 開啟 Chrome → `chrome://extensions/`
2. 右上角開啟「**開發人員模式**」
3. 點擊「**載入未封裝項目**」→ 選擇 `chatgpt-extension/` 資料夾
4. 確認 Extension 清單中出現 **ChatCMD ChatGPT Bridge**，版本 `0.1.12` 以上

> **重要：** 若之前已載入舊版 Extension，請先移除再重新載入，確保 `content-chatgpt-tool-bridge.js` 有被載入。

### 3. 設定 Extension 連線

1. 點擊 Extension 圖示 → 輸入 ChatCMD 的 **Access Code**（Local UI 上可以找到）
2. 確認狀態顯示「**已連線**」

### 4. 確認 MCP Agent 已啟用

在 ChatCMD Local UI → **MCP Agents** → 確認你要使用的 Agent 狀態為「**啟用**」。

---

## 使用方式

### 在 ChatGPT 中觸發 Tool Call

你需要讓 ChatGPT 知道它可以使用 ChatCMD 工具。在系統提示（System Prompt）或第一則訊息中說明工具呼叫協議：

````
你可以使用以下格式呼叫本機工具：

```chatcmd_tool_call
{"id":"<唯一ID>","tool":"<工具名稱>","arguments":{<參數>}}
```

呼叫後，我會把工具結果以下面格式回傳給你：

```chatcmd_tool_result
{"id":"<唯一ID>","tool":"<工具名稱>","ok":true,"content":"<結果內容>"}
```

請善用這些工具完成任務。
````

### 可用工具範例

工具名稱與參數依你的 MCP Agent 設定而定。常見工具包括：

| 工具名稱 | 功能 |
|----------|------|
| `read_file` | 讀取本機檔案 |
| `write_file` | 寫入本機檔案 |
| `run_command` | 執行指令 |
| `list_dir` | 列出目錄內容 |

> 可在 ChatCMD Local UI → **Tools** 頁面查看所有可用工具。

### 完整範例對話

**你送出：**
```
請幫我讀取 D:\project\README.md 的內容並摘要。
```

**ChatGPT 回應（Extension 自動擷取）：**
````
好的，我來讀取檔案。

```chatcmd_tool_call
{"id":"call_001","tool":"read_file","arguments":{"path":"D:\\project\\README.md"}}
```
````

**Extension 自動執行（你不需要做任何事）：**
1. 擷取 `call_001` 的 tool_call 區塊
2. 呼叫 `POST /api/local/chatgpt/bridge/tools/call`
3. ChatCMD 透過 RuntimeHost 執行 `read_file`（走完整授權/審核流程）
4. 把結果注入 ChatGPT 對話框並送出：

````
```chatcmd_tool_result
{"id":"call_001","tool":"read_file","ok":true,"content":"# My Project\n..."}
```
````

**ChatGPT 繼續回應：**
```
這份 README 的主要內容是…（摘要）
```

---

## 授權與審核（Approval）

Tool Bridge **完全走現有的 ChatCMD 授權流程**，沒有任何繞過：

- **`authorize_tool`** — 確認工具是否允許被此 Agent 使用
- **`authorize_execution`** — 若工具需要使用者審核（例如執行指令），會在 ChatCMD UI 彈出審核通知
- **Activity 記錄** — 每次工具呼叫都會出現在 Task 的 Timeline 中
- **Approval Grant** — 你可以在 ChatCMD 設定「自動允許」特定工具，避免每次都要手動審核

### 需要審核的工具

若某工具設定需要審核，ChatCMD Local UI 會顯示等待審核通知。你需要：
1. 開啟 ChatCMD Local UI
2. 在對應的 Task → Activity 中點擊「**允許**」或「**拒絕**」
3. Extension 會等待結果（最長 120 秒）後繼續

---

## 重複執行保護（Idempotency）

系統有兩層防重複機制，確保網路重試或 Extension 重載不會導致同一工具執行兩次：

| 層次 | 機制 | 範圍 |
|------|------|------|
| **第一層** | Extension 內 JS `Set` 記錄已派送的 `call_id` | 同一個 Tab 的生命週期內 |
| **第二層** | Backend 在 `timeline_events` 查詢是否已有對應 `tool_result` | 持久化，跨 Extension 重載 |

若工具已成功執行，重複的請求會直接回傳快取結果，**不會重新執行**。

---

## 受限工具

以下工具**無法**透過 Tool Bridge 呼叫（這些是 Bridge 生命週期保留工具）：

- `agent_user_message`
- `agent_completed`
- `agent_observation`
- `subagent_create`
- `subagent_result`

若 ChatGPT 嘗試呼叫這些工具，Backend 會回傳 `400 Restricted tool` 錯誤，Extension 會把錯誤格式化為 `tool_result` 回傳給 ChatGPT。

---

## 疑難排解

### Extension 沒有自動送出 tool_result

1. 確認 Extension 版本 ≥ 0.1.12（`manifest.json` 中有 `content-chatgpt-tool-bridge.js`）
2. 打開 `chrome://extensions/` → 找到 ChatCMD Bridge → 點「**背景頁面**」→ 查看 Console 錯誤
3. 在 ChatGPT Tab 按 F12 → Console → 確認沒有 `ChatCmdToolBridge` 相關錯誤

### ChatCMD 回傳 409 Bridge not started

Bridge 的 `/started` 尚未被呼叫，代表 ChatCMD 還不知道這個對話已開始。  
解決方式：確認 Extension 在收到 ChatGPT 第一則回應前，有完整執行 `started` 階段（正常情況下會自動處理）。

### ChatCMD 回傳 403 Forbidden

請求沒有帶 `X-ChatCmdClient: chatgpt-extension` 標頭。  
這通常不會發生於正常的 Extension 流程，若你是直接測試 API 請手動加上此標頭。

### 工具執行失敗但 ChatGPT 繼續了

Extension 會把失敗訊息格式化為：
```json
{"id":"call_001","tool":"read_file","ok":false,"error":"Tool execution failed: ..."}
```
ChatGPT 會讀到錯誤訊息，通常會自行決定是否重試或告知你失敗原因。

### 想查看 Tool Call 記錄

在 ChatCMD Local UI → **Tasks** → 選取對應 Task → **Timeline**，可以看到每個工具呼叫的 `tool_call` 和 `tool_result` 事件。

---

## 架構說明（給開發者）

```
chatgpt.com Tab
├── content-chatgpt-monitor.js   監控 assistant 回應穩定性
│     └── 若 hasPendingToolCalls() → 不送 completion，繼續等待
├── content-chatgpt-tool-bridge.js  解析/執行/格式化 tool call
│     ├── parseToolCalls(text)       解析 fenced block / inline tag
│     ├── hasPendingToolCalls(text)  是否有未派送的 call
│     └── runPendingCalls(...)       平行執行，注入結果，submit
└── content-chatgpt.js           主生命週期
      └── waitForAssistant 完成後 → runPendingCalls → waitForAssistant → reportResult

Background Service Worker
├── background.js
│     └── chatcmd-chatgpt-tool-call → handleToolCall()
└── background-io.js
      ├── handleProgress()    (原有，stage pipeline)
      └── handleToolCall()    (新增，完全獨立，120s timeout)

ChatCMD Backend (Rust)
└── POST /api/local/chatgpt/bridge/tools/call
      └── browser_tool_call()
            ├── 從 chatgpt_bridge_requests 取得 agent_id/task_id/turn_id
            ├── 後端冪等性檢查（timeline_events 查詢）
            └── state.runtime.call(tool, context, arguments)
                  └── RuntimeHost::call()
                        ├── authorize_tool
                        ├── ensure_call_identity
                        ├── authorize_execution / approval
                        ├── activities.register
                        ├── timeline persistence
                        └── dispatch (實際工具執行)
```

---

## 相關檔案

| 檔案 | 說明 |
|------|------|
| [`src/api/chatgpt_browser_tools.rs`](src/api/chatgpt_browser_tools.rs) | Backend 工具呼叫處理器 |
| [`src/api/routes.rs`](src/api/routes.rs) | 路由（`/chatgpt/bridge/tools/call`） |
| [`chatgpt-extension/content-chatgpt-tool-bridge.js`](chatgpt-extension/content-chatgpt-tool-bridge.js) | 解析/執行/格式化 tool call |
| [`chatgpt-extension/content-chatgpt-monitor.js`](chatgpt-extension/content-chatgpt-monitor.js) | Completion 保護（deferred to bridge） |
| [`chatgpt-extension/content-chatgpt.js`](chatgpt-extension/content-chatgpt.js) | 主生命週期（tool bridge loop） |
| [`chatgpt-extension/background-io.js`](chatgpt-extension/background-io.js) | `handleToolCall()` 函式 |
| [`chatgpt-extension/background.js`](chatgpt-extension/background.js) | Message routing |
| [`chatgpt-extension/manifest.json`](chatgpt-extension/manifest.json) | Extension 清單（load order） |
