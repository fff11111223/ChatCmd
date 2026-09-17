---
name: chatcmd-web-bridge
description: Use when interacting with a local Windows machine through the ChatCMD Web Bridge. Defines the ChatCMD tool catalog, requires exact registered tool keys, prevents invented tool names, and enforces strict sequential tool execution with unique incrementing call IDs.
---

# ChatCMD Web Bridge

Use this skill whenever interacting with the local Windows machine through the ChatCMD Web Bridge.

This skill defines:

- ChatCMD's available tool capabilities
- how to identify the actual registered tool key
- how to construct ChatCMD tool calls
- how to execute commands sequentially
- how to process tool results
- how to avoid inventing or confusing tool names
- how to use unique sequential call IDs

## 1. ChatCMD Tool Catalog

ChatCMD provides multiple tool capability groups.

### Device

Local execution-device information and operations.

### Terminal

Persistent terminal / PTY operations:

- create
- write
- wait
- read
- signal
- resize
- list
- inspect
- close

### Files and workspace

Workspace and filesystem operations:

- roots
- list
- find
- search
- read
- create
- replace
- write
- inspect
- copy
- move
- delete

### Git

Git repository operations:

- status
- diff
- log
- branches
- show revisions
- create commits

### Processes

Local process operations:

- list
- inspect
- terminate processes
- terminate process trees

### Skills

Skill discovery and reading, including Skills stored in:

- `.agents`
- `.codex`

### Tasks and orchestration

Task and agent orchestration capabilities:

- turns
- progress
- execution mode
- artifacts
- plan questions
- sub-agents
- waits
- completion

These are capability descriptions.

They are NOT automatically the actual tool-call names.

## 2. Authoritative Tool Catalog

The actual ChatCMD runtime Tool Catalog is authoritative.

It determines:

- exact tool keys
- tool IDs
- tool descriptions
- argument schemas
- enabled/disabled state
- agent tool allowlists

When the runtime catalog is available, use it instead of guessing from capability names.

If this Skill and the current ChatCMD runtime disagree, prefer the current runtime.

Never invent a tool key.

## 3. Tool Key vs Capability Name

A capability name is not necessarily the registered tool key.

For example:

Capability:

`run command`

Actual registered tool key:

`command_run`

Correct:

```json
{"tool":"command_run"}
```

Incorrect:

```json
{"tool":"run_command"}
```

Never rename, translate, abbreviate, or reorder a registered tool key.

Do not assume names such as:

- `file_read`
- `file_write`
- `process_list`
- `git_status`
- `terminal_create`

exist unless the ChatCMD runtime catalog actually reports those exact keys.

## 4. Tool Key vs Tool ID

A ChatCMD tool may have both a tool key and an internal tool ID.

Example:

Tool key:

`command_run`

Tool ID:

`tool-command_run`

When issuing a ChatGPT Web Bridge call, always use the TOOL KEY.

Correct:

```json
{"tool":"command_run"}
```

Incorrect:

```json
{"tool":"tool-command_run"}
```

## 5. Confirmed Command Tool

The confirmed ChatCMD command execution tool is:

`command_run`

It executes commands on the local Windows machine.

Required arguments:

- `executable`
- `arguments`
- `cwd`

Example:

```chatcmd_tool_call
{"id":"call_001","tool":"command_run","arguments":{"executable":"powershell","arguments":["-Command","Get-Location"],"cwd":"D:\\frank\\gemini\\ChatCmd-main\\ChatCmd-main"}}
```

For PowerShell commands, prefer:

```json
{
  "executable": "powershell",
  "arguments": ["-Command", "<PowerShell command>"],
  "cwd": "<working directory>"
}
```

`arguments` MUST be an array.

## 6. Exact Tool Call Format

Every ChatCMD Web Bridge tool call MUST use exactly this structure:

```chatcmd_tool_call
{"id":"<unique sequential ID>","tool":"<actual registered tool key>","arguments":{...}}
```

Critical formatting rules:

- The code fence MUST be exactly `chatcmd_tool_call`.
- The JSON MUST be valid.
- `id` MUST be inside the JSON object.
- `tool` MUST contain the actual registered ChatCMD tool key.
- `arguments` MUST contain the arguments required by that tool.
- NEVER put `id` after the code-fence name.
- NEVER use `run_command` as an alias for `command_run`.
- NEVER invent a tool key.

Correct:

```chatcmd_tool_call
{"id":"call_001","tool":"command_run","arguments":{"executable":"powershell","arguments":["-Command","Get-Location"],"cwd":"D:\\frank\\gemini\\ChatCmd-main\\ChatCmd-main"}}
```

Incorrect:

```chatcmd_tool_call id="call_001"
{"tool":"command_run","arguments":{}}
```

Incorrect:

```chatcmd_tool_call
{"id":"call_001","tool":"run_command","arguments":{}}
```

## 7. Unique Sequential Call IDs

Every tool call MUST have a unique ID.

IDs MUST use this exact sequential format:

```text
call_001
call_002
call_003
call_004
call_005
...
```

The number MUST increase by exactly 1 for every new tool call.

The sequence starts at:

`call_001`

for a new sequential tool-loop conversation.

Rules:

- NEVER reuse a previous call ID.
- NEVER skip a number.
- NEVER reset the sequence after receiving a result.
- NEVER use random IDs.
- NEVER use UUIDs.
- NEVER use descriptive IDs.
- NEVER use `...`.
- The ID MUST be inside the JSON object.

Example:

```chatcmd_tool_call
{"id":"call_001","tool":"command_run","arguments":{...}}
```

After receiving the result:

```chatcmd_tool_call
{"id":"call_002","tool":"command_run","arguments":{...}}
```

Then:

```chatcmd_tool_call
{"id":"call_003","tool":"command_run","arguments":{...}}
```

The sequence MUST continue:

```text
call_001
→ chatcmd_tool_result
→ call_002
→ chatcmd_tool_result
→ call_003
→ chatcmd_tool_result
→ ...
```

If a call fails:

```text
call_001 → failed
call_002 → retry
call_003 → next command
```

A failed call still consumes its ID.

## 8. Strict Sequential Tool Loop

For an autonomous diagnostic or command loop:

1. Emit ONE `chatcmd_tool_call`.
2. Wait for its `chatcmd_tool_result`.
3. Inspect the actual result.
4. Decide the next command.
5. Emit ONE new `chatcmd_tool_call`.
6. Wait for its result.
7. Repeat.

The `chatcmd_tool_result` is the synchronization barrier.

Never issue a dependent command before the previous result arrives.

Never assume a command succeeded without a result.

Never claim a command was executed unless an actual result was returned.

## 9. Do Not Batch Dependent Commands

Distinguish:

### Batch / parallel

```text
call A
call B
call C
→ results
```

from:

### Sequential agent loop

```text
call A
↓
result A
↓
analyze result A
↓
call B
↓
result B
↓
analyze result B
↓
call C
```

When the user asks to:

- investigate
- diagnose
- inspect
- check in a loop
- "下指令迴圈查"
- continue based on the previous result

use the sequential agent loop.

Do NOT put multiple dependent commands into one tool call.

The next command should be based on the actual previous result.

## 10. Handling Tool Failures

If a tool result contains:

```json
{"ok":false,"error":"Tool execution failed."}
```

do not immediately repeat the same complex command.

Simplify the command first.

Prefer:

- one directory instead of the entire repository
- one known file instead of recursive scanning
- a narrower `Select-String`
- a smaller `Get-Content` range
- a simpler PowerShell pipeline
- a shorter command

For example, instead of immediately retrying an expensive recursive search:

```powershell
Get-ChildItem . -Recurse | Select-String ...
```

first target a known file:

```powershell
Select-String -Path .\src\some_file.rs -Pattern "pattern"
```

For timeouts, use the next sequential ID and retry with a smaller/faster command.

## 11. Diagnostic Style

Prefer narrow, targeted commands.

Good:

```powershell
Select-String -Path .\chatgpt-extension\content-chatgpt-tool-bridge.js -Pattern 'runPendingCalls|executeToolCall'
```

Good:

```powershell
Get-Content .\src\runtime_host.rs | Select-Object -Skip 380 -First 40
```

Avoid unnecessarily expensive recursive scans across the entire repository when the relevant file is already known.

When source code is needed:

1. Identify the likely file.
2. Inspect a small relevant section.
3. Use the result to determine the next location.
4. Continue sequentially.

## 12. Evidence-Based Execution

Every dependent command MUST be based on actual evidence.

Do not assume:

- a file exists
- a process exists
- a path exists
- a command succeeded
- a previous modification worked
- a service is running
- a tool is available

unless the previous tool result establishes it.

Use:

```text
actual result
→ analysis
→ next command
```

not:

```text
assumption
→ next command
```

## 13. Agent Tool Allowlist

A tool can exist in ChatCMD but still be unavailable to the current agent.

These are different states:

```text
Tool exists
```

and:

```text
Tool is allowed for this agent
```

If execution returns:

```text
agent tool allowlist denied this operation
```

interpret this as an authorization/allowlist problem.

Do not change the tool name merely because of an allowlist error.

## 14. Tool Discovery

When the user asks what tools are available:

- identify the current ChatCMD Tool Catalog;
- list the available registered tools;
- explain their purposes;
- provide their argument schemas when known.

If the exact registered tool key is unknown:

1. Do NOT guess.
2. Inspect the ChatCMD runtime catalog if available.
3. Use the exact registered key returned by ChatCMD.
4. Use its corresponding argument schema.

Capability names such as `read`, `write`, `list`, `process list`, or `git status` MUST NOT automatically be treated as tool keys.

## 15. ChatGPT Web Bridge Architecture

The tool execution flow is:

```text
ChatGPT
  ↓
chatcmd_tool_call
  ↓
Chrome extension
  ↓
ChatCMD Web Bridge
  ↓
/api/local/chatgpt/bridge/tools/call
  ↓
RuntimeHost::call()
  ↓
registered ChatCMD tool
  ↓
chatcmd_tool_result
  ↓
ChatGPT
```

The browser extension parses `chatcmd_tool_call` blocks and sends them through ChatCMD.

The ChatCMD runtime is responsible for actual tool authorization and dispatch.

## 16. Multi-Turn Tool Execution

A tool result is not merely output.

It is the synchronization point between tool calls.

Correct:

```text
ChatGPT
↓
call_001
↓
tool result
↓
analyze
↓
call_002
↓
tool result
↓
analyze
↓
call_003
```

Incorrect:

```text
ChatGPT
↓
call_001
↓
call_002
↓
call_003
```

when calls 002 and 003 depend on the result of call 001.

Always wait for the actual result.

## 17. Pre-Flight Checklist

Before emitting EVERY tool call, verify:

- [ ] Code fence is exactly `chatcmd_tool_call`
- [ ] JSON is valid
- [ ] `id` is inside JSON
- [ ] ID follows `call_NNN` format
- [ ] ID is unique
- [ ] ID is exactly the next sequential number
- [ ] `tool` is an actual registered ChatCMD tool key
- [ ] Tool key has not been invented
- [ ] Required arguments are present
- [ ] `arguments` matches the tool's schema
- [ ] `command_run` uses `executable`
- [ ] `command_run` uses an array for `arguments`
- [ ] `command_run` has `cwd`
- [ ] Previous dependent call has already returned `chatcmd_tool_result`
- [ ] The command is based on actual returned evidence

## 18. Core Rules

1. ChatCMD has multiple tool groups; `command_run` is only one confirmed tool.
2. The runtime Tool Catalog is authoritative.
3. Capability names are not necessarily tool keys.
4. Tool keys and tool IDs are different.
5. Never invent a tool key.
6. `command_run` is the confirmed command execution tool key.
7. `run_command` is NOT a valid alias.
8. Every call MUST use `chatcmd_tool_call`.
9. Every call ID MUST be unique.
10. IDs MUST increment exactly: `call_001`, `call_002`, `call_003`, ...
11. A failed call still consumes its ID.
12. Never skip or reuse an ID.
13. Issue one dependent tool call at a time.
14. Wait for `chatcmd_tool_result`.
15. Base the next command on the actual result.
16. Never claim execution without an actual tool result.
17. Prefer narrow, fast diagnostic commands.
18. If exact tool information is unknown, discover it instead of guessing.