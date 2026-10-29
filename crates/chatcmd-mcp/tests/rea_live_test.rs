use std::collections::HashMap;
use std::path::Path;

use chatcmd_mcp::client::{
    ExternalServerConfig, ExternalTransportConfig, McpClientConnection, StdioTransportConfig,
    mcp_response_to_value,
};
use serde_json::json;

#[tokio::test]
async fn test_rea_live_integration() {
    let node_exe = r"C:\Program Files\nodejs\node.exe";
    let rea_script = r"D:\frank\REA\rea-package-unpacked\package\scripts\rea.mjs";

    if !Path::new(node_exe).is_file() || !Path::new(rea_script).is_file() {
        eprintln!("REA files not found at expected paths; skipping live REA test");
        return;
    }

    println!("[REA Test] Node.js executable: {node_exe}");
    println!("[REA Test] REA script: {rea_script}");

    let config = ExternalServerConfig {
        id: "rea".to_string(),
        name: "REA".to_string(),
        description: Some("Reverse Engineering Assistant live test".to_string()),
        enabled: true,
        transport: ExternalTransportConfig::Stdio(StdioTransportConfig {
            command: node_exe.to_string(),
            args: vec![rea_script.to_string(), "mcp".to_string()],
            env: HashMap::new(),
            cwd: None,
        }),
        timeout_secs: 60,
        auto_connect: true,
    };

    let mut conn = McpClientConnection::new(config);

    // 1. Connect & initialize
    println!("[REA Test] Connecting to REA via stdio...");
    let tools = match conn.connect().await {
        Ok(t) => t,
        Err(e) => {
            panic!("Failed to connect to REA: {e}");
        }
    };

    println!("[REA Test] Connected! Status: {:?}", conn.status());
    println!("[REA Test] Tool count: {}", tools.len());
    assert!(
        tools.len() > 10,
        "REA should provide many reverse engineering tools (found {})",
        tools.len()
    );

    // 2. Measure schema size
    let schema_json = serde_json::to_string(&tools).unwrap_or_default();
    let schema_bytes = schema_json.len();
    println!(
        "[REA Test] Total tools/list schema size: {schema_bytes} bytes ({:.2} KB)",
        schema_bytes as f64 / 1024.0
    );

    // Print first 5 tool names
    for tool in tools.iter().take(5) {
        println!("  - {}: {}", tool.qualified_name, tool.description);
    }

    // 3. Call a non-destructive tool: 'list_documents'
    let doc_tool = tools.iter().find(|t| t.original_name == "list_documents");
    if let Some(t) = doc_tool {
        println!(
            "[REA Test] Calling non-destructive tool '{}'...",
            t.original_name
        );
        match conn.call_tool(&t.original_name, json!({})).await {
            Ok(resp) => {
                let val = mcp_response_to_value(resp);
                println!("[REA Test] Tool call result: {}", val);
            }
            Err(e) => {
                println!("[REA Test] Tool call returned error (expected if no binary loaded): {e}");
            }
        }
    }

    // 4. Clean disconnect
    println!("[REA Test] Disconnecting...");
    conn.disconnect().await;
    assert!(!conn.is_connected());
    println!("[REA Test] Disconnected cleanly.");
}
