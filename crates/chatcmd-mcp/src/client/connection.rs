use rmcp::model::{CallToolRequestParams, CallToolResponse, PaginatedRequestParams};
use rmcp::service::{RoleClient, RunningService, serve_client};
use serde_json::Value;
use std::time::Duration;

use super::handler::GenericClientHandler;
use super::model::{
    ExternalServerConfig, ExternalServerStatus, ExternalToolDefinition, ExternalTransportConfig,
    format_qualified_tool_name,
};
use super::transport::{create_http_transport, create_stdio_transport};

/// An active client connection to an external MCP server.
pub struct McpClientConnection {
    config: ExternalServerConfig,
    running: Option<RunningService<RoleClient, GenericClientHandler>>,
    status: ExternalServerStatus,
}

impl McpClientConnection {
    pub fn new(config: ExternalServerConfig) -> Self {
        Self {
            config,
            running: None,
            status: ExternalServerStatus::Disconnected,
        }
    }

    pub fn config(&self) -> &ExternalServerConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: ExternalServerConfig) {
        self.config = config;
    }

    pub fn status(&self) -> &ExternalServerStatus {
        &self.status
    }

    pub fn is_connected(&self) -> bool {
        self.status.is_connected() && self.running.is_some()
    }

    /// Connect to the external MCP server and perform initialization handshake.
    pub async fn connect(&mut self) -> Result<Vec<ExternalToolDefinition>, String> {
        self.status = ExternalServerStatus::Connecting;
        let handler = GenericClientHandler::default();

        let running = match &self.config.transport {
            ExternalTransportConfig::Stdio(stdio) => {
                let transport = create_stdio_transport(stdio).map_err(|e| {
                    format!("Failed to start stdio process '{}': {e}", stdio.command)
                })?;
                serve_client(handler, transport).await.map_err(|e| {
                    format!(
                        "MCP initialization failed for stdio server '{}': {e}",
                        self.config.id
                    )
                })?
            }
            ExternalTransportConfig::StreamableHttp(http) => {
                let transport = create_http_transport(http).map_err(|e| {
                    format!("Failed to configure HTTP transport for '{}': {e}", http.url)
                })?;
                serve_client(handler, transport).await.map_err(|e| {
                    format!(
                        "MCP initialization failed for HTTP server '{}': {e}",
                        self.config.id
                    )
                })?
            }
        };

        let now_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()) as i64;

        let peer_info = running.peer().peer_info();
        let (protocol_version, server_name, server_version) = if let Some(info) = peer_info {
            (
                info.protocol_version.to_string(),
                info.server_info.as_ref().map(|s| s.name.clone()),
                info.server_info.as_ref().map(|s| s.version.clone()),
            )
        } else {
            ("unknown".to_string(), None, None)
        };

        self.running = Some(running);
        self.status = ExternalServerStatus::Connected {
            protocol_version,
            server_name,
            server_version,
            connected_at_ms: now_ms,
        };

        // Fetch tool list
        match self.fetch_tools().await {
            Ok(tools) => Ok(tools),
            Err(e) => {
                self.status = ExternalServerStatus::Error {
                    message: format!("Connected but failed to list tools: {e}"),
                    occurred_at_ms: now_ms,
                };
                Err(e)
            }
        }
    }

    /// Retrieve all tools from the connected server, handling pagination if required.
    pub async fn fetch_tools(&self) -> Result<Vec<ExternalToolDefinition>, String> {
        let running = self
            .running
            .as_ref()
            .ok_or_else(|| "Server is not connected".to_string())?;

        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0;
        const MAX_PAGES: usize = 50;

        loop {
            pages += 1;
            if pages > MAX_PAGES {
                tracing::warn!(server_id = %self.config.id, "Tools pagination exceeded maximum page limit");
                break;
            }

            let params = cursor.map(|c| PaginatedRequestParams::default().with_cursor(Some(c)));

            let timeout = Duration::from_secs(self.config.timeout_secs.max(10));
            let result = tokio::time::timeout(timeout, running.peer().list_tools(params))
                .await
                .map_err(|_| format!("list_tools timed out after {}s", self.config.timeout_secs))?
                .map_err(|e| format!("list_tools error: {e}"))?;

            for tool in result.tools {
                let original_name = tool.name.to_string();
                let qualified_name = format_qualified_tool_name(&self.config.id, &original_name);
                let display_name = tool.title.unwrap_or_else(|| original_name.clone());
                let description = tool.description.map(|d| d.to_string()).unwrap_or_default();
                let input_schema = serde_json::to_value(&tool.input_schema).unwrap_or(Value::Null);

                tools.push(ExternalToolDefinition {
                    server_id: self.config.id.clone(),
                    original_name,
                    qualified_name,
                    display_name,
                    description,
                    input_schema,
                    annotations: None,
                });
            }

            match result.next_cursor {
                Some(next) if !next.trim().is_empty() => cursor = Some(next.to_string()),
                _ => break,
            }
        }

        Ok(tools)
    }

    /// Call a tool by its original MCP name on this server.
    pub async fn call_tool(
        &self,
        original_name: &str,
        arguments: Value,
    ) -> Result<CallToolResponse, String> {
        let running = self
            .running
            .as_ref()
            .ok_or_else(|| format!("Server '{}' is not connected", self.config.id))?;

        let args_obj = match arguments {
            Value::Object(map) => Some(map),
            Value::Null => None,
            other => {
                return Err(format!("Arguments must be a JSON object, got {}", other));
            }
        };

        let mut params = CallToolRequestParams::new(original_name.to_string());
        if let Some(obj) = args_obj {
            params = params.with_arguments(obj);
        }

        let timeout = Duration::from_secs(self.config.timeout_secs.max(1));
        let response = tokio::time::timeout(timeout, running.call_tool_once(params))
            .await
            .map_err(|_| {
                format!(
                    "Tool execution timed out after {}s",
                    self.config.timeout_secs
                )
            })?
            .map_err(|e| format!("MCP tool call failed: {e}"))?;

        Ok(response)
    }

    /// Disconnect from the server and terminate child process/connections.
    pub async fn disconnect(&mut self) {
        if let Some(running) = self.running.take() {
            // Drop running service triggers child process kill & transport close
            drop(running);
        }
        self.status = ExternalServerStatus::Disconnected;
    }
}

/// Convert an MCP `CallToolResponse` into a `serde_json::Value` for the bridge.
/// Preserves all content types. If single non-error text content, returns plain string.
pub fn mcp_response_to_value(response: CallToolResponse) -> Value {
    match response {
        CallToolResponse::Complete(result) => {
            let is_error = result.is_error.unwrap_or(false);
            let val = serde_json::to_value(&result).unwrap_or(Value::Null);
            if !is_error {
                if let Some(content) = val.get("content").and_then(|c| c.as_array()) {
                    if content.len() == 1 {
                        if let Some(text) = content[0].get("text").and_then(|t| t.as_str()) {
                            return Value::String(text.to_string());
                        }
                    }
                }
            }
            val
        }
        CallToolResponse::InputRequired(input_req) => {
            serde_json::to_value(&input_req).unwrap_or(Value::Null)
        }
        CallToolResponse::Task(task) => serde_json::to_value(&task).unwrap_or(Value::Null),
        _ => Value::Null,
    }
}
