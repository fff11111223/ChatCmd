use std::collections::HashMap;
use std::sync::RwLock;

use super::model::{ExternalToolDefinition, ExternalToolSummary};

/// In-memory catalog indexing all external tools from all active MCP servers.
#[derive(Debug, Default)]
pub struct ExternalToolCatalog {
    tools_by_qualified_name: RwLock<HashMap<String, ExternalToolDefinition>>,
    tool_names_by_server: RwLock<HashMap<String, Vec<String>>>,
}

impl ExternalToolCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace all tools registered for a given server.
    pub fn register_server_tools(&self, server_id: &str, tools: Vec<ExternalToolDefinition>) {
        let mut by_name = self.tools_by_qualified_name.write().unwrap();
        let mut by_server = self.tool_names_by_server.write().unwrap();

        // Remove old tools for this server
        if let Some(old_names) = by_server.remove(server_id) {
            for name in old_names {
                by_name.remove(&name);
            }
        }

        let mut registered_names = Vec::with_capacity(tools.len());
        for tool in tools {
            registered_names.push(tool.qualified_name.clone());
            by_name.insert(tool.qualified_name.clone(), tool);
        }

        by_server.insert(server_id.to_string(), registered_names);
    }

    /// Remove all tools for a given server.
    pub fn unregister_server(&self, server_id: &str) {
        let mut by_name = self.tools_by_qualified_name.write().unwrap();
        let mut by_server = self.tool_names_by_server.write().unwrap();

        if let Some(names) = by_server.remove(server_id) {
            for name in names {
                by_name.remove(&name);
            }
        }
    }

    /// Retrieve a tool by its qualified name (`mcp__{server_id}__{tool_name}`).
    pub fn get_tool(&self, qualified_name: &str) -> Option<ExternalToolDefinition> {
        let by_name = self.tools_by_qualified_name.read().unwrap();
        by_name.get(qualified_name).cloned()
    }

    /// Look up a tool by server_id and original tool name.
    pub fn find_tool(
        &self,
        server_id: &str,
        original_name: &str,
    ) -> Option<ExternalToolDefinition> {
        let qualified = super::model::format_qualified_tool_name(server_id, original_name);
        self.get_tool(&qualified)
    }

    /// Search tools across servers by keyword query with optional server filtering and pagination.
    pub fn search_tools(
        &self,
        query: &str,
        server_filter: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> (Vec<ExternalToolSummary>, usize) {
        let query_lower = query.trim().to_lowercase();
        let by_name = self.tools_by_qualified_name.read().unwrap();

        let mut matches = Vec::new();
        for tool in by_name.values() {
            if let Some(server) = server_filter {
                if !tool.server_id.eq_ignore_ascii_case(server.trim()) {
                    continue;
                }
            }

            if query_lower.is_empty()
                || tool.qualified_name.to_lowercase().contains(&query_lower)
                || tool.original_name.to_lowercase().contains(&query_lower)
                || tool.display_name.to_lowercase().contains(&query_lower)
                || tool.description.to_lowercase().contains(&query_lower)
            {
                matches.push(ExternalToolSummary {
                    server_id: tool.server_id.clone(),
                    original_name: tool.original_name.clone(),
                    qualified_name: tool.qualified_name.clone(),
                    display_name: tool.display_name.clone(),
                    description: tool.description.clone(),
                });
            }
        }

        // Sort alphabetically for deterministic results
        matches.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));

        let total = matches.len();
        let paged = matches.into_iter().skip(offset).take(limit).collect();
        (paged, total)
    }

    /// Get all tools for a specific server.
    pub fn list_server_tools(&self, server_id: &str) -> Vec<ExternalToolDefinition> {
        let by_server = self.tool_names_by_server.read().unwrap();
        let by_name = self.tools_by_qualified_name.read().unwrap();

        let mut results = Vec::new();
        if let Some(names) = by_server.get(server_id) {
            for name in names {
                if let Some(tool) = by_name.get(name) {
                    results.push(tool.clone());
                }
            }
        }
        results.sort_by(|a, b| a.original_name.cmp(&b.original_name));
        results
    }

    /// Count how many tools are registered for a given server.
    pub fn server_tools_count(&self, server_id: &str) -> usize {
        let by_server = self.tool_names_by_server.read().unwrap();
        by_server.get(server_id).map_or(0, |v| v.len())
    }

    /// Total count of all external tools across all servers.
    pub fn total_count(&self) -> usize {
        let by_name = self.tools_by_qualified_name.read().unwrap();
        by_name.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_tool(server_id: &str, tool_name: &str, desc: &str) -> ExternalToolDefinition {
        let qualified_name = crate::client::model::format_qualified_tool_name(server_id, tool_name);
        ExternalToolDefinition {
            server_id: server_id.to_string(),
            original_name: tool_name.to_string(),
            qualified_name,
            display_name: tool_name.to_string(),
            description: desc.to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
            annotations: None,
        }
    }

    #[test]
    fn test_catalog_multi_server_collision_and_isolation() {
        let catalog = ExternalToolCatalog::new();

        // Server A and Server B both have a tool named "search"
        let tools_a = vec![
            make_tool("server_a", "search", "Search in server A"),
            make_tool("server_a", "read", "Read in server A"),
        ];
        let tools_b = vec![
            make_tool("server_b", "search", "Search in server B"),
            make_tool("server_b", "write", "Write in server B"),
        ];

        catalog.register_server_tools("server_a", tools_a);
        catalog.register_server_tools("server_b", tools_b);

        assert_eq!(catalog.total_count(), 4);
        assert_eq!(catalog.server_tools_count("server_a"), 2);
        assert_eq!(catalog.server_tools_count("server_b"), 2);

        // Disambiguated by qualified name
        let tool_a = catalog.get_tool("mcp__server_a__search").unwrap();
        assert_eq!(tool_a.description, "Search in server A");

        let tool_b = catalog.get_tool("mcp__server_b__search").unwrap();
        assert_eq!(tool_b.description, "Search in server B");

        // Find by server and original name
        let found = catalog.find_tool("server_a", "read").unwrap();
        assert_eq!(found.qualified_name, "mcp__server_a__read");

        // Unregister server A
        catalog.unregister_server("server_a");
        assert_eq!(catalog.total_count(), 2);
        assert_eq!(catalog.server_tools_count("server_a"), 0);
        assert_eq!(catalog.server_tools_count("server_b"), 2);
        assert!(catalog.get_tool("mcp__server_a__search").is_none());
        assert!(catalog.get_tool("mcp__server_b__search").is_some());
    }

    #[test]
    fn test_catalog_search_and_pagination() {
        let catalog = ExternalToolCatalog::new();

        let tools = vec![
            make_tool(
                "rea",
                "analyze_function",
                "Disassemble and analyze native procedure",
            ),
            make_tool("rea", "inspect_pe", "Inspect Portable Executable headers"),
            make_tool("rea", "list_procedures", "Enumerate functions in binary"),
            make_tool("git_mcp", "git_status", "Check working copy status"),
            make_tool("git_mcp", "git_log", "Inspect git commit history"),
        ];
        catalog.register_server_tools("mixed", tools);

        // Search query "inspect"
        let (results, total) = catalog.search_tools("inspect", None, 10, 0);
        assert_eq!(total, 2);
        assert_eq!(results.len(), 2);

        // Search with server filter
        let (results, total) = catalog.search_tools("inspect", Some("rea"), 10, 0);
        assert_eq!(total, 1);
        assert_eq!(results[0].original_name, "inspect_pe");

        // Search with pagination
        let (paged, total) = catalog.search_tools("", None, 2, 0);
        assert_eq!(total, 5);
        assert_eq!(paged.len(), 2);

        let (paged2, total2) = catalog.search_tools("", None, 2, 2);
        assert_eq!(total2, 5);
        assert_eq!(paged2.len(), 2);
        assert_ne!(paged[0].qualified_name, paged2[0].qualified_name);
    }
}
