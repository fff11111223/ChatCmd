use std::collections::HashMap;
use std::path::PathBuf;

use chatcmd_mcp::client::{
    ExternalServerConfig, ExternalToolCatalog, ExternalTransportConfig, McpClientConnection,
    StdioTransportConfig, mcp_response_to_value,
};
use serde_json::json;

fn resolve_node_exe() -> Option<String> {
    let standard = r"C:\Program Files\nodejs\node.exe";
    if std::path::Path::new(standard).is_file() {
        return Some(standard.to_string());
    }
    // Fallback to "node" in PATH
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("node".to_string());
    }
    None
}

fn mock_server_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("mock_stdio_server.mjs")
}

#[tokio::test]
async fn test_stdio_mock_mcp_server_lifecycle() {
    let node = match resolve_node_exe() {
        Some(n) => n,
        None => {
            eprintln!("Node.js not available; skipping stdio integration test");
            return;
        }
    };

    let script = mock_server_script_path();
    assert!(script.is_file(), "mock_stdio_server.mjs should exist");

    let config = ExternalServerConfig {
        id: "mock_test".to_string(),
        name: "Mock Server".to_string(),
        description: Some("Test mock stdio server".to_string()),
        enabled: true,
        transport: ExternalTransportConfig::Stdio(StdioTransportConfig {
            command: node,
            args: vec![script.display().to_string()],
            env: HashMap::new(),
            cwd: None,
        }),
        timeout_secs: 15,
        auto_connect: true,
    };

    let mut conn = McpClientConnection::new(config);
    assert!(!conn.is_connected());

    // 1. Connect & discover tools
    let tools = conn
        .connect()
        .await
        .expect("connection to mock stdio server should succeed");

    assert!(conn.is_connected());
    assert_eq!(tools.len(), 2);

    let echo_tool = tools.iter().find(|t| t.original_name == "echo").unwrap();
    assert_eq!(echo_tool.qualified_name, "mcp__mock_test__echo");
    assert_eq!(echo_tool.description, "Echoes back input message");

    let add_tool = tools.iter().find(|t| t.original_name == "add").unwrap();
    assert_eq!(add_tool.qualified_name, "mcp__mock_test__add");

    // 2. Register tools in catalog
    let catalog = ExternalToolCatalog::new();
    catalog.register_server_tools("mock_test", tools);
    assert_eq!(catalog.total_count(), 2);

    // 3. Call tool 'echo'
    let echo_resp = conn
        .call_tool("echo", json!({ "message": "hello world" }))
        .await
        .expect("echo call should succeed");

    let echo_val = mcp_response_to_value(echo_resp);
    assert_eq!(echo_val, json!("echo: hello world"));

    // 4. Call tool 'add'
    let add_resp = conn
        .call_tool("add", json!({ "a": 15, "b": 27 }))
        .await
        .expect("add call should succeed");

    let add_val = mcp_response_to_value(add_resp);
    assert_eq!(add_val, json!("42"));

    // 5. Disconnect
    conn.disconnect().await;
    assert!(!conn.is_connected());
    catalog.unregister_server("mock_test");
    assert_eq!(catalog.total_count(), 0);
}

#[tokio::test]
async fn test_multi_server_catalog_routing() {
    let catalog = ExternalToolCatalog::new();

    // Server 1: rea
    let rea_tools = vec![
        chatcmd_mcp::client::ExternalToolDefinition {
            server_id: "rea".to_string(),
            original_name: "analyze_function".to_string(),
            qualified_name: "mcp__rea__analyze_function".to_string(),
            display_name: "Analyze Function".to_string(),
            description: "Analyze native binary function".to_string(),
            input_schema: json!({ "type": "object" }),
            annotations: None,
        },
        chatcmd_mcp::client::ExternalToolDefinition {
            server_id: "rea".to_string(),
            original_name: "binary_overview".to_string(),
            qualified_name: "mcp__rea__binary_overview".to_string(),
            display_name: "Binary Overview".to_string(),
            description: "Binary overview and architecture".to_string(),
            input_schema: json!({ "type": "object" }),
            annotations: None,
        },
    ];

    // Server 2: ghidra (hypothetical future MCP server)
    let ghidra_tools = vec![chatcmd_mcp::client::ExternalToolDefinition {
        server_id: "ghidra".to_string(),
        original_name: "analyze_function".to_string(), // Same tool name as REA!
        qualified_name: "mcp__ghidra__analyze_function".to_string(),
        display_name: "Ghidra Analyze".to_string(),
        description: "Ghidra decompiler procedure analysis".to_string(),
        input_schema: json!({ "type": "object" }),
        annotations: None,
    }];

    catalog.register_server_tools("rea", rea_tools);
    catalog.register_server_tools("ghidra", ghidra_tools);

    // Verify tools from both servers coexist without collision
    assert_eq!(catalog.total_count(), 3);
    assert_eq!(catalog.server_tools_count("rea"), 2);
    assert_eq!(catalog.server_tools_count("ghidra"), 1);

    // Look up disambiguated tools
    let rea_tool = catalog.get_tool("mcp__rea__analyze_function").unwrap();
    assert_eq!(rea_tool.server_id, "rea");
    assert_eq!(rea_tool.description, "Analyze native binary function");

    let ghidra_tool = catalog.get_tool("mcp__ghidra__analyze_function").unwrap();
    assert_eq!(ghidra_tool.server_id, "ghidra");
    assert_eq!(
        ghidra_tool.description,
        "Ghidra decompiler procedure analysis"
    );

    // Search specifically filters by server
    let (rea_results, rea_total) = catalog.search_tools("analyze", Some("rea"), 10, 0);
    assert_eq!(rea_total, 1);
    assert_eq!(rea_results[0].server_id, "rea");

    let (all_results, all_total) = catalog.search_tools("analyze", None, 10, 0);
    assert_eq!(all_total, 2);
    assert_eq!(all_results.len(), 2);
}
