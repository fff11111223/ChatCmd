use chatcmd_core::{McpAgentStore as _, ToolCatalogStore as _};
use chatcmd_runtime::{RuntimeError, RuntimeResult};

use super::super::{RuntimeHost, invalid, storage_error};

impl RuntimeHost {
    pub(crate) async fn authorize_tool(
        &self,
        agent_id: &str,
        tool: &str,
    ) -> RuntimeResult<()> {
        let id = chatcmd_core::AgentId::new(agent_id)
            .map_err(|_| invalid("agentId", "must be a non-empty string"))?;
        self.repository
            .agent(&id)
            .await
            .map_err(storage_error)?
            .filter(|agent| agent.enabled)
            .ok_or_else(|| RuntimeError::new("unauthorized", "agent is disabled or missing"))?;
        if matches!(
            tool,
            "agent_user_message"
                | "agent_progress"
                | "agent_plan_question"
                | "agent_subagent_start"
                | "agent_subagent_wait"
                | "agent_turn_complete"
        ) {
            return Ok(());
        }
        let allowed = self
            .repository
            .agent_allowed_tool_ids(&id)
            .await
            .map_err(storage_error)?;
        let tools = self.repository.list_tools().await.map_err(storage_error)?;

        // 1. Direct match: e.g. "mcp_servers_list", "mcp_tools_search", "mcp_tool_schema", or exact tool key
        if tools.iter().any(|candidate| {
            candidate.key == tool && candidate.enabled && allowed.contains(&candidate.id)
        }) {
            return Ok(());
        }

        // 2. Server-level match for qualified external tools: e.g. "mcp__rea__open_binary" -> check "mcp__rea"
        if let Some((server_id, _)) = chatcmd_mcp::client::parse_qualified_tool_name(tool) {
            let server_key = format!("mcp__{server_id}");
            if tools.iter().any(|candidate| {
                candidate.key == server_key && candidate.enabled && allowed.contains(&candidate.id)
            }) {
                return Ok(());
            }
        }

        Err(RuntimeError::new(
            "policy_denied",
            "agent tool allowlist denied this operation",
        ))
    }
}
