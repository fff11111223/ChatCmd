use chatcmd_core::{
    McpAgentStore as _, ToolCapability, ToolCatalogStore as _, ToolDefinition, ToolGroup,
    ToolPreset,
};
use chatcmd_storage::SqliteRepository;

pub(super) async fn seed_catalog(
    repository: &SqliteRepository,
) -> Result<(), chatcmd_core::StorageError> {
    let groups = vec![
        tool_group("group-device", "device", "Device", 10),
        tool_group("group-terminal", "terminal", "Terminal", 20),
        tool_group("group-files", "files", "Files & workspace", 30),
        tool_group("group-git", "git", "Git", 40),
        tool_group("group-process", "process", "Processes", 50),
        tool_group("group-skills", "skills", "Skills", 60),
        tool_group("group-tasks", "tasks", "Tasks & agent lifecycle", 70),
        tool_group("group-mcp", "mcp", "External MCP (外部 MCP)", 80),
    ];
    let mut tools = chatcmd_mcp::TOOL_NAMES
        .iter()
        .map(|name| ToolDefinition {
            id: seeded_tool_id(name),
            key: name.clone(),
            group_id: tool_group_id(name).to_owned(),
            title: name.replace('_', " "),
            description: format!("Local {name} operation"),
            input_schema_json: "{}".to_owned(),
            capabilities: if [
                "fs_delete",
                "fs_move",
                "git_commit",
                "process_kill",
                "shell_close",
            ]
            .contains(&name.as_str())
            {
                vec![ToolCapability::Destructive]
            } else if name.starts_with("blob_")
                || name.starts_with("fs_write")
                || matches!(name.as_str(), "fs_replace_text" | "fs_apply_edits")
            {
                vec![ToolCapability::Write]
            } else {
                vec![ToolCapability::Read]
            },
            enabled: true,
        })
        .collect::<Vec<_>>();

    // Seed MCP discovery and external tools so they appear in Agent permissions UI
    tools.push(ToolDefinition {
        id: "tool-mcp-servers-list".to_string(),
        key: "mcp_servers_list".to_string(),
        group_id: "group-mcp".to_string(),
        title: "mcp servers list".to_string(),
        description: "List configured external MCP servers and their status".to_string(),
        input_schema_json: "{}".to_string(),
        capabilities: vec![ToolCapability::Read],
        enabled: true,
    });
    tools.push(ToolDefinition {
        id: "tool-mcp-tools-search".to_string(),
        key: "mcp_tools_search".to_string(),
        group_id: "group-mcp".to_string(),
        title: "mcp tools search".to_string(),
        description: "Search external MCP tools by keyword".to_string(),
        input_schema_json: "{}".to_string(),
        capabilities: vec![ToolCapability::Read],
        enabled: true,
    });
    tools.push(ToolDefinition {
        id: "tool-mcp-tool-schema".to_string(),
        key: "mcp_tool_schema".to_string(),
        group_id: "group-mcp".to_string(),
        title: "mcp tool schema".to_string(),
        description: "View parameter schema of an external MCP tool".to_string(),
        input_schema_json: "{}".to_string(),
        capabilities: vec![ToolCapability::Read],
        enabled: true,
    });
    tools.push(ToolDefinition {
        id: "tool-mcp-server-rea".to_string(),
        key: "mcp__rea".to_string(),
        group_id: "group-mcp".to_string(),
        title: "REA (External MCP Server)".to_string(),
        description: "Allow calling reverse engineering tools on REA MCP server".to_string(),
        input_schema_json: "{}".to_string(),
        capabilities: vec![ToolCapability::Read],
        enabled: true,
    });

    let safe_ids = tools
        .iter()
        .filter(|tool| !tool.capabilities.contains(&ToolCapability::Destructive))
        .map(|tool| tool.id.clone())
        .collect();
    let presets = vec![ToolPreset {
        id: "preset-safe".to_owned(),
        key: "safe".to_owned(),
        name: "Safe local tools".to_owned(),
        description: "All non-destructive local tools".to_owned(),
        tool_ids: safe_ids,
    }];
    repository
        .replace_catalog(&groups, &tools, &presets)
        .await?;

    let write_id = seeded_tool_id("fs_write_text");
    let replace_id = seeded_tool_id("fs_replace_text");
    let apply_edits_id = seeded_tool_id("fs_apply_edits");
    let mcp_ids = [
        "tool-mcp-servers-list",
        "tool-mcp-tools-search",
        "tool-mcp-tool-schema",
        "tool-mcp-server-rea",
    ];
    for agent in repository.list_agents().await? {
        let mut allowed = repository.agent_allowed_tool_ids(&agent.id).await?;
        let mut changed = false;
        if allowed.contains(&write_id) && !allowed.contains(&replace_id) {
            allowed.push(replace_id.clone());
            changed = true;
        }
        if allowed.contains(&write_id) && !allowed.contains(&apply_edits_id) {
            allowed.push(apply_edits_id.clone());
            changed = true;
        }
        // If an agent has skills_list (meaning it's an interactive assistant like worker),
        // initialize it with MCP tools so they appear as enabled by default.
        if allowed.contains(&seeded_tool_id("skills_list")) {
            for mcp_id in &mcp_ids {
                if !allowed.contains(&mcp_id.to_string()) {
                    allowed.push(mcp_id.to_string());
                    changed = true;
                }
            }
        }
        if changed {
            repository
                .set_agent_allowed_tools(&agent.id, &allowed)
                .await?;
        }
    }
    Ok(())
}

fn tool_group(id: &str, key: &str, display_name: &str, sort_order: i32) -> ToolGroup {
    ToolGroup {
        id: id.to_owned(),
        key: key.to_owned(),
        display_name: display_name.to_owned(),
        sort_order,
    }
}

fn tool_group_id(name: &str) -> &'static str {
    if name.starts_with("device_") {
        "group-device"
    } else if name.starts_with("shell_") {
        "group-terminal"
    } else if name.starts_with("fs_") || name.starts_with("blob_") || name == "workspace_roots" {
        "group-files"
    } else if name.starts_with("git_") {
        "group-git"
    } else if name.starts_with("process_") || name == "command_run" {
        "group-process"
    } else if name.starts_with("skill_") || name.starts_with("skills_") {
        "group-skills"
    } else {
        "group-tasks"
    }
}

fn seeded_tool_id(name: &str) -> String {
    match name {
        "device_list" => "tool-device-list".to_owned(),
        "shell_create" => "tool-shell-create".to_owned(),
        "shell_read" => "tool-shell-read".to_owned(),
        "shell_write" => "tool-shell-write".to_owned(),
        "fs_read_text" => "tool-fs-read".to_owned(),
        _ => format!("tool-{name}"),
    }
}
