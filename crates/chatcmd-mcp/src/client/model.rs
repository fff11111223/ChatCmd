use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Server identifier: must be non-empty and contain alphanumeric, underscore, or hyphen.
pub fn validate_server_id(id: &str) -> Result<(), String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("Server ID cannot be empty".to_string());
    }
    if trimmed.len() > 64 {
        return Err("Server ID cannot exceed 64 characters".to_string());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(
            "Server ID may only contain letters, numbers, underscores, and hyphens".to_string(),
        );
    }
    Ok(())
}

/// Naming convention for external MCP tools exposed to ChatGPT and the bridge:
/// `mcp__{server_id}__{tool_name}`
pub fn format_qualified_tool_name(server_id: &str, tool_name: &str) -> String {
    format!("mcp__{}__{}", server_id.trim(), tool_name.trim())
}

/// Reversible parser for `mcp__{server_id}__{tool_name}`.
pub fn parse_qualified_tool_name(qualified_name: &str) -> Option<(&str, &str)> {
    let without_prefix = qualified_name.strip_prefix("mcp__")?;
    without_prefix.split_once("__")
}

fn default_true() -> bool {
    true
}

fn default_timeout_secs() -> u64 {
    30
}

/// Configuration for an external MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub transport: ExternalTransportConfig,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_true")]
    pub auto_connect: bool,
}

impl ExternalServerConfig {
    pub fn validate(&self) -> Result<(), String> {
        validate_server_id(&self.id)?;
        if self.name.trim().is_empty() {
            return Err("Server name cannot be empty".to_string());
        }
        self.transport.validate()?;
        Ok(())
    }
}

/// Transport-specific settings for connecting to an external MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExternalTransportConfig {
    Stdio(StdioTransportConfig),
    StreamableHttp(StreamableHttpTransportConfig),
}

impl ExternalTransportConfig {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Stdio(s) => s.validate(),
            Self::StreamableHttp(h) => h.validate(),
        }
    }

    pub fn transport_type(&self) -> &'static str {
        match self {
            Self::Stdio(_) => "stdio",
            Self::StreamableHttp(_) => "streamable_http",
        }
    }
}

/// Stdio subprocess connection settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StdioTransportConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

impl StdioTransportConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.command.trim().is_empty() {
            return Err("Executable command cannot be empty".to_string());
        }
        Ok(())
    }
}

/// Streamable HTTP connection settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamableHttpTransportConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl StreamableHttpTransportConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.url.trim().is_empty() {
            return Err("HTTP endpoint URL cannot be empty".to_string());
        }
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err("HTTP endpoint URL must start with http:// or https://".to_string());
        }
        Ok(())
    }
}

/// Dynamic connection and health status of an external MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ExternalServerStatus {
    Disconnected,
    Connecting,
    Connected {
        protocol_version: String,
        #[serde(default)]
        server_name: Option<String>,
        #[serde(default)]
        server_version: Option<String>,
        connected_at_ms: i64,
    },
    Error {
        message: String,
        occurred_at_ms: i64,
    },
}

impl ExternalServerStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
}

/// Definition of an external tool discovered from an MCP server via tools/list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalToolDefinition {
    /// ID of the origin MCP server.
    pub server_id: String,
    /// Tool name as reported by the MCP server.
    pub original_name: String,
    /// Globally unique, collision-free name exposed externally (`mcp__{server_id}__{original_name}`).
    pub qualified_name: String,
    /// Display title or fallback to original name.
    pub display_name: String,
    /// Tool description from server schema.
    pub description: String,
    /// Full JSON inputSchema from server.
    pub input_schema: Value,
    /// Optional annotations or extra metadata provided by the server.
    #[serde(default)]
    pub annotations: Option<Value>,
}

/// High-level summary of a tool for search/listing without transmitting full schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalToolSummary {
    pub server_id: String,
    pub original_name: String,
    pub qualified_name: String,
    pub display_name: String,
    pub description: String,
}

/// Summary of an external MCP server including its runtime status and tool count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalServerSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub transport_type: String,
    pub status: ExternalServerStatus,
    pub tools_count: usize,
    pub updated_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_server_id() {
        assert!(validate_server_id("rea").is_ok());
        assert!(validate_server_id("my-server_01").is_ok());
        assert!(validate_server_id("").is_err());
        assert!(validate_server_id("   ").is_err());
        assert!(validate_server_id("server.with.dots").is_err());
        assert!(validate_server_id("server with spaces").is_err());
        assert!(validate_server_id("server/slash").is_err());
        let too_long = "a".repeat(65);
        assert!(validate_server_id(&too_long).is_err());
    }

    #[test]
    fn test_format_and_parse_qualified_tool_name() {
        let qname = format_qualified_tool_name("rea", "analyze_function");
        assert_eq!(qname, "mcp__rea__analyze_function");

        let parsed = parse_qualified_tool_name(&qname);
        assert_eq!(parsed, Some(("rea", "analyze_function")));

        // Edge case: tool name contains underscores
        let qname2 = format_qualified_tool_name("test_server", "get_file_info");
        assert_eq!(qname2, "mcp__test_server__get_file_info");
        let parsed2 = parse_qualified_tool_name(&qname2);
        assert_eq!(parsed2, Some(("test_server", "get_file_info")));

        // Invalid tool names
        assert_eq!(parse_qualified_tool_name("not_mcp_tool"), None);
        assert_eq!(parse_qualified_tool_name("mcp__missing_split"), None);
        assert_eq!(parse_qualified_tool_name(""), None);
    }

    #[test]
    fn test_config_validation() {
        let valid_stdio = ExternalServerConfig {
            id: "rea".to_string(),
            name: "REA".to_string(),
            description: Some("Reverse Engineering Assistant".to_string()),
            enabled: true,
            transport: ExternalTransportConfig::Stdio(StdioTransportConfig {
                command: "node".to_string(),
                args: vec!["rea.mjs".to_string(), "mcp".to_string()],
                env: Default::default(),
                cwd: None,
            }),
            timeout_secs: 30,
            auto_connect: true,
        };
        assert!(valid_stdio.validate().is_ok());

        // Empty command
        let empty_cmd = ExternalServerConfig {
            transport: ExternalTransportConfig::Stdio(StdioTransportConfig {
                command: "".to_string(),
                args: vec![],
                env: Default::default(),
                cwd: None,
            }),
            ..valid_stdio.clone()
        };
        assert!(empty_cmd.validate().is_err());

        // Invalid ID
        let bad_id = ExternalServerConfig {
            id: "bad id with spaces".to_string(),
            ..valid_stdio.clone()
        };
        assert!(bad_id.validate().is_err());

        // HTTP validation
        let valid_http = ExternalServerConfig {
            id: "remote".to_string(),
            name: "Remote".to_string(),
            description: None,
            enabled: true,
            transport: ExternalTransportConfig::StreamableHttp(StreamableHttpTransportConfig {
                url: "https://example.com/mcp".to_string(),
                headers: Default::default(),
            }),
            timeout_secs: 30,
            auto_connect: false,
        };
        assert!(valid_http.validate().is_ok());

        let invalid_http = ExternalServerConfig {
            transport: ExternalTransportConfig::StreamableHttp(StreamableHttpTransportConfig {
                url: "ftp://example.com".to_string(),
                headers: Default::default(),
            }),
            ..valid_http
        };
        assert!(invalid_http.validate().is_err());
    }
}
