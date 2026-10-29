use std::collections::HashMap;
use std::sync::Arc;

pub use chatcmd_mcp::client::mcp_response_to_value;
use chatcmd_mcp::client::{
    ExternalServerConfig, ExternalServerStatus, ExternalServerSummary, ExternalToolCatalog,
    ExternalToolDefinition, ExternalToolSummary, McpClientConnection, parse_qualified_tool_name,
};
use serde_json::Value;
use tokio::sync::RwLock;

/// Manages lifecycle of all external MCP server connections and the unified tool catalog.
pub struct ExternalMcpManager {
    /// All known server connections, keyed by server ID.
    connections: RwLock<HashMap<String, McpClientConnection>>,
    /// Unified in-memory index of all tools across all connected servers.
    catalog: Arc<ExternalToolCatalog>,
}

impl ExternalMcpManager {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            catalog: Arc::new(ExternalToolCatalog::new()),
        }
    }

    #[allow(dead_code)]
    pub fn catalog(&self) -> Arc<ExternalToolCatalog> {
        self.catalog.clone()
    }

    // ── Config management ──────────────────────────────────────────────────

    /// Add or replace a server config without connecting.
    pub async fn upsert_config(&self, config: ExternalServerConfig) {
        let mut conns = self.connections.write().await;
        conns
            .entry(config.id.clone())
            .and_modify(|c| c.update_config(config.clone()))
            .or_insert_with(|| McpClientConnection::new(config));
    }

    /// Remove a server and disconnect it.
    pub async fn remove_server(&self, server_id: &str) {
        let mut conns = self.connections.write().await;
        if let Some(mut conn) = conns.remove(server_id) {
            conn.disconnect().await;
        }
        self.catalog.unregister_server(server_id);
    }

    // ── Connection management ───────────────────────────────────────────────

    /// Connect a server by ID. Returns tool count on success.
    pub async fn connect(&self, server_id: &str) -> Result<usize, String> {
        let mut conns = self.connections.write().await;
        let conn = conns
            .get_mut(server_id)
            .ok_or_else(|| format!("Server '{server_id}' not found"))?;

        if conn.is_connected() {
            return Ok(self.catalog.server_tools_count(server_id));
        }

        let tools = conn.connect().await?;
        let count = tools.len();
        self.catalog.register_server_tools(server_id, tools);
        Ok(count)
    }

    /// Disconnect a server by ID.
    pub async fn disconnect(&self, server_id: &str) -> Result<(), String> {
        let mut conns = self.connections.write().await;
        let conn = conns
            .get_mut(server_id)
            .ok_or_else(|| format!("Server '{server_id}' not found"))?;
        conn.disconnect().await;
        self.catalog.unregister_server(server_id);
        Ok(())
    }

    /// Refresh tools for a connected server (re-fetch tools/list).
    #[allow(dead_code)]
    pub async fn refresh_tools(&self, server_id: &str) -> Result<usize, String> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(server_id)
            .ok_or_else(|| format!("Server '{server_id}' not found"))?;
        let tools = conn.fetch_tools().await?;
        let count = tools.len();
        drop(conns);
        self.catalog.register_server_tools(server_id, tools);
        Ok(count)
    }

    // ── Tool dispatch ───────────────────────────────────────────────────────

    /// Route a qualified tool call (`mcp__{server_id}__{tool_name}`) to the correct MCP server.
    pub async fn call_tool(&self, qualified_name: &str, arguments: Value) -> Result<Value, String> {
        let (server_id, original_name) = parse_qualified_tool_name(qualified_name)
            .ok_or_else(|| format!("Invalid qualified tool name: '{qualified_name}'"))?;

        let conns = self.connections.read().await;
        let conn = conns
            .get(server_id)
            .ok_or_else(|| format!("Server '{server_id}' not found"))?;

        if !conn.is_connected() {
            return Err(format!("Server '{server_id}' is not connected"));
        }

        let response = conn.call_tool(original_name, arguments).await?;
        Ok(mcp_response_to_value(response))
    }

    // ── Queries ─────────────────────────────────────────────────────────────

    pub async fn list_servers(&self) -> Vec<ExternalServerSummary> {
        let conns = self.connections.read().await;
        let mut result: Vec<ExternalServerSummary> = conns
            .values()
            .map(|conn| {
                let config = conn.config();
                ExternalServerSummary {
                    id: config.id.clone(),
                    name: config.name.clone(),
                    description: config.description.clone(),
                    enabled: config.enabled,
                    transport_type: config.transport.transport_type().to_string(),
                    status: conn.status().clone(),
                    tools_count: self.catalog.server_tools_count(&config.id),
                    updated_at_ms: 0,
                }
            })
            .collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }

    pub async fn get_server_config(&self, server_id: &str) -> Option<ExternalServerConfig> {
        let conns = self.connections.read().await;
        conns.get(server_id).map(|c| c.config().clone())
    }

    pub async fn get_server_status(&self, server_id: &str) -> Option<ExternalServerStatus> {
        let conns = self.connections.read().await;
        conns.get(server_id).map(|c| c.status().clone())
    }

    pub async fn server_exists(&self, server_id: &str) -> bool {
        let conns = self.connections.read().await;
        conns.contains_key(server_id)
    }

    pub fn search_tools(
        &self,
        query: &str,
        server_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> (Vec<ExternalToolSummary>, usize) {
        self.catalog.search_tools(query, server_id, limit, offset)
    }

    pub fn get_tool_schema(&self, qualified_name: &str) -> Option<ExternalToolDefinition> {
        self.catalog.get_tool(qualified_name)
    }

    pub fn list_server_tools(&self, server_id: &str) -> Vec<ExternalToolDefinition> {
        self.catalog.list_server_tools(server_id)
    }

    // ── Startup helpers ─────────────────────────────────────────────────────

    /// Load persisted server configs and auto-connect enabled ones.
    pub async fn load_and_connect_all(&self, configs: Vec<ExternalServerConfig>) {
        for config in configs {
            let id = config.id.clone();
            let auto_connect = config.enabled && config.auto_connect;
            self.upsert_config(config).await;
            if auto_connect {
                let id_clone = id.clone();
                if let Err(e) = self.connect(&id_clone).await {
                    tracing::warn!(server_id = %id_clone, error = %e, "Failed to auto-connect external MCP server on startup");
                } else {
                    let count = self.catalog.server_tools_count(&id_clone);
                    tracing::info!(server_id = %id_clone, tools = count, "Connected external MCP server on startup");
                }
            }
        }
    }
}

/// Default REA server configuration — paths are configurable in UI, not hardcoded in core logic.
pub fn default_rea_config() -> ExternalServerConfig {
    use chatcmd_mcp::client::{ExternalTransportConfig, StdioTransportConfig};
    ExternalServerConfig {
        id: "rea".to_string(),
        name: "REA".to_string(),
        description: Some("Reverse Engineering Assistant v6.7.0".to_string()),
        enabled: false, // disabled by default until user enables it
        transport: ExternalTransportConfig::Stdio(StdioTransportConfig {
            command: r"C:\Program Files\nodejs\node.exe".to_string(),
            args: vec![
                r"D:\frank\REA\rea-package-unpacked\package\scripts\rea.mjs".to_string(),
                "mcp".to_string(),
            ],
            env: std::collections::HashMap::new(),
            cwd: None,
        }),
        timeout_secs: 60,
        auto_connect: true,
    }
}
