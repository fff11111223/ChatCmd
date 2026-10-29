use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use chatcmd_mcp::RuntimeApi;
use chatcmd_runtime::OperationContext;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use crate::websocket::AppState;

use super::{Problem, db_problem};

/// Blocked lifecycle tool names that must not be called through the browser
/// tool bridge (they are managed by the bridge lifecycle handlers).
const BLOCKED_TOOLS: &[&str] = &[
    "agent_user_message",
    "agent_completed",
    "agent_observation",
    "agent_subagent_start",
    "subagent_create",
    "subagent_result",
];

/// Maximum byte length of a call_id to guard against absurdly large values.
const MAX_CALL_ID_BYTES: usize = 240;
/// Maximum command output size returned to the AI through the browser bridge.
/// This does not change command execution or persisted results.
const MAX_AI_COMMAND_RESULT_BYTES: usize = 64 * 1024;

fn guard_command_run_result_for_ai(tool: &str, value: Value) -> Value {
    if tool != "command_run" {
        return value;
    }

    let object = match value.as_object() {
        Some(object) => object,
        None => return value,
    };

    let stdout_bytes = object
        .get("stdoutBytes")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            object
                .get("stdout")
                .and_then(Value::as_str)
                .map_or(0, |s| s.len() as u64)
        });
    let stderr_bytes = object
        .get("stderrBytes")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            object
                .get("stderr")
                .and_then(Value::as_str)
                .map_or(0, |s| s.len() as u64)
        });
    let output_bytes = stdout_bytes.saturating_add(stderr_bytes);

    if output_bytes <= MAX_AI_COMMAND_RESULT_BYTES as u64 {
        return value;
    }

    json!({
        "resultTooLarge": true,
        "originalStdoutBytes": stdout_bytes,
        "originalStderrBytes": stderr_bytes,
        "originalOutputBytes": output_bytes,
        "maxAiResultBytes": MAX_AI_COMMAND_RESULT_BYTES,
        "executionId": object.get("executionId").cloned().unwrap_or(Value::Null),
        "exitCode": object.get("exitCode").cloned().unwrap_or(Value::Null),
        "terminalState": object.get("terminalState").cloned().unwrap_or(Value::Null),
        "truncated": object.get("truncated").cloned().unwrap_or(Value::Null),
        "warning": "The command_run result was too large to return in full. Do not request the complete output. Use Select-String, Select-Object -First/-Last, split the output into chunks, or specify file/line ranges, then issue another command_run. An oversized result does NOT mean the task is complete. You MUST continue with another command_run."
    })
}

/// Body sent by `content-chatgpt-tool-bridge.js` for each tool call.
///
/// The extension does NOT supply `agentId`, `taskId`, or `turnId` — those
/// are loaded from `chatgpt_bridge_requests` using the authoritative
/// `requestId`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BrowserToolCall {
    /// Matches the `id` column in `chatgpt_bridge_requests`.
    pub request_id: String,

    /// A stable, per-tool-invocation ID supplied by ChatGPT tool-call
    /// protocol.  Used as the `OperationContext::request_id` so that
    /// `append_call_event()` idempotency keys are stable across retries.
    pub call_id: String,

    /// The MCP tool name to execute.
    pub tool: String,

    /// Optional note provided by the model for this tool call.
    #[serde(default)]
    pub note: Option<String>,

    /// Tool arguments -- must be a JSON object.
    #[serde(default)]
    pub arguments: Value,
}

pub(super) async fn browser_tool_call(
    State(state): State<Arc<AppState>>,
    Json(input): Json<BrowserToolCall>,
) -> Result<Json<Value>, Problem> {
    // 1. Validate call_id
    let call_id = input.call_id.trim();
    if call_id.is_empty() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Missing call ID",
            "callId is required and must be non-empty.",
        ));
    }
    if call_id.len() > MAX_CALL_ID_BYTES {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid call ID",
            "callId must not exceed 240 bytes.",
        ));
    }

    // 2. Validate tool
    let tool = input.tool.trim();
    if tool.is_empty() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Missing tool",
            "tool is required.",
        ));
    }
    if BLOCKED_TOOLS.contains(&tool) {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Restricted tool",
            "This tool cannot be called through the browser tool bridge.",
        ));
    }

    // 3. Validate arguments
    if !input.arguments.is_object() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid arguments",
            "arguments must be a JSON object.",
        ));
    }

    // 4. Validate request_id
    let request_id = input.request_id.trim();
    if request_id.is_empty() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Missing request ID",
            "requestId is required.",
        ));
    }

    // 5. Load authoritative identity from chatgpt_bridge_requests
    let row = super::chatgpt_support::bridge_request_row(&state, request_id).await?;

    let status: String = row.get("status");
    if !matches!(status.as_str(), "queued" | "running") {
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "Bridge request not active",
            "The ChatGPT bridge request is not active. Only 'queued' or 'running' requests accept tool calls.",
        ));
    }

    let task_id: Option<String> = row.get("task_id");
    if task_id.is_none() {
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "Bridge not started",
            "The bridge request has no task yet. Ensure /bridge/{id}/started was called first.",
        ));
    }

    let agent_id: String = row.get("agent_id");
    let turn_id: String = row.get("turn_id");
    let conversation_id: Option<String> = row.get("conversation_id");
    let task_id = task_id.unwrap();

    let project_folder_str: Option<String> = match row.get::<Option<String>, _>("project_folder") {
        Some(pf) if !pf.trim().is_empty() => Some(pf),
        _ => sqlx::query_scalar("SELECT project_folder FROM tasks WHERE id=?")
            .bind(&task_id)
            .fetch_optional(state.repository.pool())
            .await
            .ok()
            .flatten(),
    };
    let project_folder = project_folder_str.as_deref().map(std::path::Path::new);
    let resident_footer = match state
        .skills
        .active_resident_instructions_for_task(Some(&task_id), project_folder)
        .await
    {
        Ok((instructions, _warnings)) => {
            let trimmed = instructions.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    };

    // 6. Backend idempotency: if a tool_result already exists for this
    //    call_id, return it immediately without re-executing.
    let existing_result: Option<String> = sqlx::query_scalar(
        "SELECT payload_json FROM timeline_events \
         WHERE task_id=? AND turn_id=? AND actor='assistant' AND kind='tool_result' \
         AND json_extract(payload_json,'$.activityId')=? LIMIT 1",
    )
    .bind(&task_id)
    .bind(&turn_id)
    .bind(call_id)
    .fetch_optional(state.repository.pool())
    .await
    .map_err(db_problem)?;

    let note_to_save = match input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        Some(n) => Some(n.to_owned()),
        None => {
            let think_payload: Option<String> = sqlx::query_scalar(
                "SELECT payload_json FROM timeline_events WHERE task_id=? AND kind='chatgpt_think' ORDER BY created_at_ms DESC LIMIT 1",
            )
            .bind(&task_id)
            .fetch_optional(state.repository.pool())
            .await
            .ok()
            .flatten();
            think_payload.and_then(|p| {
                crate::api::task_views::extract_note_for_call_from_think_json(call_id, &p)
            })
        }
    };

    if let Some(payload_json) = existing_result {
        if let Some(ref note) = note_to_save {
            let _ = sqlx::query(
                "UPDATE timeline_events SET payload_json = json_set(payload_json, '$.note', ?) WHERE task_id = ? AND json_extract(payload_json, '$.activityId') = ?",
            )
            .bind(note)
            .bind(&task_id)
            .bind(call_id)
            .execute(state.repository.pool())
            .await;
        }
        let stored: Value = serde_json::from_str(&payload_json).unwrap_or(Value::Null);
        let result_content = guard_command_run_result_for_ai(
            tool,
            stored.get("content").cloned().unwrap_or(Value::Null),
        );
        return Ok(Json(json!({
            "ok": true,
            "idempotent": true,
            "callId": call_id,
            "requestId": request_id,
            "taskId": task_id,
            "turnId": turn_id,
            "tool": tool,
            "result": result_content,
            "residentFooter": resident_footer,
        })));
    }

    // 7. Ensure user message is synced in timeline for this task and turn.
    // RuntimeHost::call() checks ensure_user_message_synced(), which requires
    // an actor='user' kind='message' event for this task_id and turn_id.
    let user_content: String = row.get("user_content");
    let submitted_content: String = row.get("submitted_content");
    super::chatgpt_support::append_user_message(
        &state,
        &task_id,
        &turn_id,
        request_id,
        &user_content,
        &submitted_content,
    )
    .await?;

    // 8. Build OperationContext with authoritative identity
    //
    // call_id is used as request_id so RuntimeHost idempotency keys are
    // stable per tool call across retries.
    let mut context = OperationContext::new(call_id.to_owned(), agent_id, tool.to_owned());
    context.task_id = Some(task_id.clone());
    context.turn_id = Some(turn_id.clone());
    context.conversation_scope_id = conversation_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .map(|id| format!("openai:{id}"));
    // mcp_session_id intentionally left as None for browser bridge calls.

    // 8. Execute via RuntimeHost::call() -- the ONE AND ONLY entry point for local tools.
    //    For external MCP tools (mcp__{server_id}__{tool_name}), route through ExternalMcpManager.
    //    authorize_tool / ensure_call_identity / authorize_execution /
    //    approval / activity registration / timeline persistence / dispatch
    //    are all invoked through this single path for local tools.
    let result = if tool.starts_with("mcp__") {
        // External MCP tool dispatch — routes to the appropriate server connection.
        // We still record the call in the timeline via the result path below.
        let raw_result = state
            .external_mcp
            .call_tool(tool, input.arguments.clone())
            .await;

        // Apply the same byte budget as local commands. MCP text results may be large.
        let value = match raw_result {
            Ok(mut val) => {
                // Guard string results
                if let Value::String(ref s) = val {
                    if s.len() > MAX_AI_COMMAND_RESULT_BYTES {
                        val = Value::String(format!(
                            "[MCP result truncated: {} bytes, limit {} bytes]\n{}",
                            s.len(),
                            MAX_AI_COMMAND_RESULT_BYTES,
                            &s[..MAX_AI_COMMAND_RESULT_BYTES],
                        ));
                    }
                }
                Ok(val)
            }
            Err(e) => Err(chatcmd_runtime::RuntimeError::new("mcp_error", e)),
        };

        // Persist tool_result event in timeline so idempotency and history work correctly.
        if let Ok(ref result_value) = value {
            let payload = serde_json::to_string(&serde_json::json!({
                "activityId": call_id,
                "tool": tool,
                "content": result_value,
            }))
            .unwrap_or_default();
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO timeline_events \
                 (task_id, turn_id, actor, kind, payload_json, created_at_ms) \
                 VALUES (?, ?, 'assistant', 'tool_result', ?, ?)",
            )
            .bind(&task_id)
            .bind(&turn_id)
            .bind(&payload)
            .bind(super::now_ms())
            .execute(state.repository.pool())
            .await;
        }
        value
    } else {
        state.runtime.call(tool, context, input.arguments).await
    };

    if let Some(ref note) = note_to_save {
        let _ = sqlx::query(
            "UPDATE timeline_events SET payload_json = json_set(payload_json, '$.note', ?) WHERE task_id = ? AND json_extract(payload_json, '$.activityId') = ?",
        )
        .bind(note)
        .bind(&task_id)
        .bind(call_id)
        .execute(state.repository.pool())
        .await;
    }

    match result {
        Ok(value) => Ok(Json(json!({
            "ok": true,
            "idempotent": false,
            "callId": call_id,
            "requestId": request_id,
            "taskId": task_id,
            "turnId": turn_id,
            "tool": tool,
            "result": guard_command_run_result_for_ai(tool, value),
            "residentFooter": resident_footer,
        }))),

        Err(error) => Ok(Json(json!({
            "ok": false,
            "idempotent": false,
            "callId": call_id,
            "requestId": request_id,
            "taskId": task_id,
            "turnId": turn_id,
            "tool": tool,
            "error": {
                "code": error.code,
                "message": error.message,
            },
            "residentFooter": resident_footer,
        }))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[tokio::test]
    async fn browser_tool_call_rejects_subagent_start() {
        let (host, _agent_id, directory) =
            crate::runtime_host::user_message_tests::test_host().await;
        let state = Arc::new(
            host.test_app_state(directory.path().join("chatcmd.db").display().to_string()),
        );
        let payload = BrowserToolCall {
            request_id: "request-subagent-blocked".to_string(),
            call_id: "call-subagent-blocked".to_string(),
            tool: "agent_subagent_start".to_string(),
            note: None,
            arguments: json!({"name": "test", "request": "test"}),
        };

        let error = browser_tool_call(axum::extract::State(state), Json(payload))
            .await
            .expect_err("subagent registration must not bypass worker dispatch");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.title, "Restricted tool");
    }

    #[tokio::test]
    async fn browser_tool_call_includes_resident_footer_when_skill_has_resident() {
        let (host, agent_id, directory) =
            crate::runtime_host::user_message_tests::test_host().await;
        let state = Arc::new(
            host.test_app_state(directory.path().join("chatcmd.db").display().to_string()),
        );
        let now = super::super::now_ms();

        // Create global skill with resident.md
        let global_skills_dir = state.skills.global_skills_dir();
        let global_skill = global_skills_dir.join("global-resident-skill");
        fs::create_dir_all(&global_skill).expect("create global skill");
        fs::write(
            global_skill.join("core.md"),
            "---
name: global-resident-skill
description: Core
---
Core instructions
",
        )
        .expect("write core");
        fs::write(
            global_skill.join("resident.md"),
            "Strict safety rule: resident instructions active.",
        )
        .expect("write resident");

        let task_id = "task-resident-test";
        let request_id = "request-resident-test";
        sqlx::query(
            "INSERT INTO tasks(id,agent_id,device_id,title,source,status,generation,created_at_ms,updated_at_ms)              VALUES(?,?,?,'Resident test','chatgpt_web','running',1,?,?)",
        )
        .bind(task_id)
        .bind(&agent_id)
        .bind(state.device.id.as_str())
        .bind(now)
        .bind(now)
        .execute(state.repository.pool())
        .await
        .expect("seed task");
        sqlx::query(
            "INSERT INTO chatgpt_bridge_requests(id,task_id,turn_id,agent_id,model,user_content,submitted_content,status,created_at_ms,updated_at_ms)              VALUES(?,?,?,?,'Auto','hello','hello','running',?,?)",
        )
        .bind(request_id)
        .bind(task_id)
        .bind("turn-resident-test")
        .bind(&agent_id)
        .bind(now)
        .bind(now)
        .execute(state.repository.pool())
        .await
        .expect("seed request");

        // OK-ish call (may fail to read binary DB file — doesn't matter)
        let payload = BrowserToolCall {
            request_id: request_id.to_string(),
            call_id: "call_001".to_string(),
            tool: "fs_read_text".to_string(),
            note: None,
            arguments: json!({ "path": directory.path().join("chatcmd.db").to_string_lossy().to_string() }),
        };
        let response = browser_tool_call(axum::extract::State(state.clone()), Json(payload))
            .await
            .expect("tool call");
        let res_val = &response.0;
        let footer = res_val.get("residentFooter").and_then(Value::as_str);
        assert!(footer.is_some(), "residentFooter should be present");
        assert!(
            footer
                .unwrap()
                .contains("Strict safety rule: resident instructions active.")
        );

        // Error call: residentFooter still present
        let payload_err = BrowserToolCall {
            request_id: request_id.to_string(),
            call_id: "call_002".to_string(),
            tool: "fs_read_text".to_string(),
            note: None,
            arguments: json!({ "path": "non-existent-file-path-xyz" }),
        };
        let err_response =
            browser_tool_call(axum::extract::State(state.clone()), Json(payload_err))
                .await
                .expect("err call");
        let err_val = &err_response.0;
        assert_eq!(err_val.get("ok").and_then(Value::as_bool), Some(false));
        let err_footer = err_val.get("residentFooter").and_then(Value::as_str);
        assert!(
            err_footer.is_some(),
            "residentFooter should be present on error response"
        );
        assert!(err_footer.unwrap().contains("Strict safety rule"));
    }

    #[tokio::test]
    async fn browser_tool_call_resident_footer_absent_when_no_resident_skill() {
        let (host, agent_id, directory) =
            crate::runtime_host::user_message_tests::test_host().await;
        let mut state =
            host.test_app_state(directory.path().join("chatcmd.db").display().to_string());
        state.skills = chatcmd_runtime::SkillService::new(
            None,
            None,
            Some(&directory.path().join("empty-skills")),
            10_000,
        );
        let state = Arc::new(state);
        let now = super::super::now_ms();

        let task_id = "task-no-resident";
        let request_id = "request-no-resident";
        sqlx::query(
            "INSERT INTO tasks(id,agent_id,device_id,title,source,status,generation,created_at_ms,updated_at_ms)              VALUES(?,?,?,'No resident','chatgpt_web','running',1,?,?)",
        )
        .bind(task_id)
        .bind(&agent_id)
        .bind(state.device.id.as_str())
        .bind(now)
        .bind(now)
        .execute(state.repository.pool())
        .await
        .expect("seed task");
        sqlx::query(
            "INSERT INTO chatgpt_bridge_requests(id,task_id,turn_id,agent_id,model,user_content,submitted_content,status,created_at_ms,updated_at_ms)              VALUES(?,?,?,?,'Auto','hello','hello','running',?,?)",
        )
        .bind(request_id)
        .bind(task_id)
        .bind("turn-no-resident")
        .bind(&agent_id)
        .bind(now)
        .bind(now)
        .execute(state.repository.pool())
        .await
        .expect("seed request");

        let payload = BrowserToolCall {
            request_id: request_id.to_string(),
            call_id: "call_010".to_string(),
            tool: "fs_read_text".to_string(),
            note: None,
            arguments: json!({ "path": "non-existent" }),
        };
        let response = browser_tool_call(axum::extract::State(state.clone()), Json(payload))
            .await
            .expect("call");
        let val = &response.0;
        let footer = val.get("residentFooter");
        assert!(
            footer.map_or(true, |v| v.is_null()),
            "residentFooter should be null when no resident skills"
        );
    }
}
