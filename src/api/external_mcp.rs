use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chatcmd_mcp::client::ExternalServerConfig;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::websocket::AppState;

use super::Problem;

// ── Helpers ────────────────────────────────────────────────────────────────

fn mcp_problem(msg: impl Into<String>) -> Problem {
    Problem::new(StatusCode::BAD_REQUEST, "MCP Error", msg.into())
}
fn mcp_not_found(id: &str) -> Problem {
    Problem::new(
        StatusCode::NOT_FOUND,
        "Not found",
        format!("External MCP server '{id}' not found"),
    )
}

// ── Persistence helpers ─────────────────────────────────────────────────────

const MCP_SERVERS_SETTING_KEY: &str = "external_mcp_servers";

/// Load all server configs from the settings table.
pub(crate) async fn load_configs(state: &AppState) -> Result<Vec<ExternalServerConfig>, Problem> {
    use chatcmd_core::SettingsStore as _;
    let raw = state
        .repository
        .setting(MCP_SERVERS_SETTING_KEY)
        .await
        .map_err(super::storage_problem)?;
    match raw {
        Some(s) => {
            let configs: Vec<ExternalServerConfig> =
                serde_json::from_str(&s.value_json).unwrap_or_default();
            Ok(configs)
        }
        None => Ok(Vec::new()),
    }
}

async fn save_configs(state: &AppState, configs: &[ExternalServerConfig]) -> Result<(), Problem> {
    use chatcmd_core::{Setting, SettingsStore as _};
    let json_str = serde_json::to_string(configs).map_err(|e| {
        Problem::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Serialization error",
            e.to_string(),
        )
    })?;
    let setting = Setting {
        key: MCP_SERVERS_SETTING_KEY.to_string(),
        value_json: json_str,
        updated_at_ms: super::now_ms(),
    };
    state
        .repository
        .set_setting(&setting)
        .await
        .map_err(super::storage_problem)
}

// ── GET /external-mcp/servers ───────────────────────────────────────────────

pub(super) async fn list_external_servers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, Problem> {
    let servers = state.external_mcp.list_servers().await;
    Ok(Json(json!({ "servers": servers })))
}

// ── POST /external-mcp/servers ──────────────────────────────────────────────

pub(super) async fn create_external_server(
    State(state): State<Arc<AppState>>,
    Json(config): Json<ExternalServerConfig>,
) -> Result<Json<Value>, Problem> {
    config.validate().map_err(mcp_problem)?;

    if state.external_mcp.server_exists(&config.id).await {
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "Conflict",
            format!("Server '{}' already exists", config.id),
        ));
    }

    // Persist first
    let mut configs = load_configs(&state).await?;
    configs.retain(|c| c.id != config.id);
    configs.push(config.clone());
    save_configs(&state, &configs).await?;

    // Register in manager
    state.external_mcp.upsert_config(config.clone()).await;

    Ok(Json(json!({ "server": config })))
}

// ── PUT /external-mcp/servers/{id} ─────────────────────────────────────────

pub(super) async fn update_external_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut config): Json<ExternalServerConfig>,
) -> Result<Json<Value>, Problem> {
    // Ensure the URL path ID wins
    config.id = id.clone();
    config.validate().map_err(mcp_problem)?;

    if !state.external_mcp.server_exists(&id).await {
        return Err(mcp_not_found(&id));
    }

    // If connected, disconnect first so the new config takes effect on next connect
    let _ = state.external_mcp.disconnect(&id).await;

    let mut configs = load_configs(&state).await?;
    configs.retain(|c| c.id != id);
    configs.push(config.clone());
    save_configs(&state, &configs).await?;

    state.external_mcp.upsert_config(config.clone()).await;

    Ok(Json(json!({ "server": config })))
}

// ── DELETE /external-mcp/servers/{id} ──────────────────────────────────────

pub(super) async fn delete_external_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Problem> {
    if !state.external_mcp.server_exists(&id).await {
        return Err(mcp_not_found(&id));
    }

    state.external_mcp.remove_server(&id).await;

    let mut configs = load_configs(&state).await?;
    configs.retain(|c| c.id != id);
    save_configs(&state, &configs).await?;

    Ok(Json(json!({ "deleted": true, "id": id })))
}

// ── POST /external-mcp/servers/{id}/connect ────────────────────────────────

pub(super) async fn connect_external_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Problem> {
    if !state.external_mcp.server_exists(&id).await {
        return Err(mcp_not_found(&id));
    }

    let tool_count = state.external_mcp.connect(&id).await.map_err(mcp_problem)?;

    let status = state.external_mcp.get_server_status(&id).await;
    Ok(Json(json!({
        "connected": true,
        "toolCount": tool_count,
        "status": status,
    })))
}

// ── POST /external-mcp/servers/{id}/disconnect ─────────────────────────────

pub(super) async fn disconnect_external_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Problem> {
    if !state.external_mcp.server_exists(&id).await {
        return Err(mcp_not_found(&id));
    }

    state
        .external_mcp
        .disconnect(&id)
        .await
        .map_err(mcp_problem)?;

    Ok(Json(json!({ "disconnected": true, "id": id })))
}

// ── POST /external-mcp/servers/{id}/test ───────────────────────────────────

pub(super) async fn test_external_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Problem> {
    if !state.external_mcp.server_exists(&id).await {
        return Err(mcp_not_found(&id));
    }

    // Disconnect, re-connect, report result
    let _ = state.external_mcp.disconnect(&id).await;

    match state.external_mcp.connect(&id).await {
        Ok(count) => {
            let status = state.external_mcp.get_server_status(&id).await;
            Ok(Json(json!({
                "ok": true,
                "toolCount": count,
                "status": status,
            })))
        }
        Err(e) => Ok(Json(json!({
            "ok": false,
            "error": e,
        }))),
    }
}

// ── GET /external-mcp/servers/{id}/tools ───────────────────────────────────

pub(super) async fn list_server_tools(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Problem> {
    if !state.external_mcp.server_exists(&id).await {
        return Err(mcp_not_found(&id));
    }
    let tools = state.external_mcp.list_server_tools(&id);
    Ok(Json(json!({ "tools": tools })))
}

// ── GET /external-mcp/tools/search ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct SearchQuery {
    #[serde(default)]
    pub q: String,
    #[serde(rename = "serverId")]
    pub server_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}
fn default_limit() -> usize {
    50
}

pub(super) async fn search_tools(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<Value> {
    let limit = params.limit.min(200);
    let (tools, total) = state.external_mcp.search_tools(
        &params.q,
        params.server_id.as_deref(),
        limit,
        params.offset,
    );
    Json(json!({
        "tools": tools,
        "total": total,
        "limit": limit,
        "offset": params.offset,
    }))
}

// ── GET /external-mcp/tools/{qualified_name}/schema ────────────────────────

pub(super) async fn tool_schema(
    State(state): State<Arc<AppState>>,
    Path(qualified_name): Path<String>,
) -> Result<Json<Value>, Problem> {
    match state.external_mcp.get_tool_schema(&qualified_name) {
        Some(tool) => Ok(Json(json!({ "tool": tool }))),
        None => Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Tool not found",
            format!("Tool '{qualified_name}' not found in catalog"),
        )),
    }
}

// ── GET /external-mcp/servers/{id}/config ──────────────────────────────────

pub(super) async fn get_server_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Problem> {
    match state.external_mcp.get_server_config(&id).await {
        Some(config) => Ok(Json(json!({ "server": config }))),
        None => Err(mcp_not_found(&id)),
    }
}
