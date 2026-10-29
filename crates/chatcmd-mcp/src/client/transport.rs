use rmcp::transport::child_process::TokioChildProcess;

use super::model::{StdioTransportConfig, StreamableHttpTransportConfig};

/// Creates a stdio child-process transport from configuration.
pub fn create_stdio_transport(config: &StdioTransportConfig) -> std::io::Result<TokioChildProcess> {
    let mut cmd = tokio::process::Command::new(&config.command);
    cmd.args(&config.args);
    for (key, val) in &config.env {
        cmd.env(key, val);
    }
    if let Some(ref cwd) = config.cwd {
        cmd.current_dir(cwd);
    }
    TokioChildProcess::new(cmd)
}

/// Creates a Streamable HTTP client transport from configuration.
///
/// NOTE: In Phase 1, rmcp 3.2.0 requires reqwest 0.13 for its internal StreamableHttpClientWorker,
/// which conflicts with ChatCMD's workspace-wide reqwest 0.12 dependency. To maintain dependency
/// stability without breaking changes to the workspace, Streamable HTTP is architected as an
/// extensible option in `ExternalTransportConfig::StreamableHttp` and will be activated in Phase 2
/// upon workspace reqwest upgrade.
pub fn create_http_transport(
    _config: &StreamableHttpTransportConfig,
) -> Result<TokioChildProcess, String> {
    Err(
        "Streamable HTTP client transport is currently deferred in Phase 1 due to workspace reqwest version compatibility (rmcp 3.2.0 requires reqwest 0.13). Stdio subprocess transport is fully supported."
            .to_string(),
    )
}
