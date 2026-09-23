use std::path::Path;
use chatcmd_core::SettingsStore as _;
use chatcmd_runtime::{OperationContext, RuntimeError, RuntimeResult};
use serde_json::Value;

use super::RuntimeHost;

/// Returns true for tools that access the filesystem (fs_* and workspace index tools).
/// Must stay in sync with dispatch/helpers.rs `is_filesystem_tool`.
fn is_filesystem_tool_name(tool: &str) -> bool {
    tool.starts_with("fs_") || matches!(tool, "workspace_index_status" | "workspace_index_rebuild")
}

#[derive(Default, Debug)]
pub(crate) struct SafetySettings {
    pub block_file_delete: bool,
    pub block_process_kill: bool,
    pub block_system_shutdown: bool,
    pub block_disk_format: bool,
    pub custom_blocked_keywords: Vec<String>,
    pub enforce_project_folder_only: bool,
}

impl RuntimeHost {
    async fn load_setting_bool(&self, key: &str, default: bool) -> bool {
        match self.repository.setting(&format!("ui_{key}")).await {
            Ok(Some(setting)) => serde_json::from_str::<bool>(&setting.value_json).unwrap_or(default),
            _ => default,
        }
    }

    pub(crate) async fn load_safety_settings(&self) -> SafetySettings {
        let block_file_delete = self.load_setting_bool("blockFileDelete", true).await;
        let block_process_kill = self.load_setting_bool("blockProcessKill", true).await;
        let block_system_shutdown = self.load_setting_bool("blockSystemShutdown", true).await;
        let block_disk_format = self.load_setting_bool("blockDiskFormat", true).await;
        let enforce_project_folder_only = self.load_setting_bool("enforceProjectFolderOnly", true).await;

        let custom_blocked_keywords = match self.repository.setting("ui_customBlockedKeywords").await {
            Ok(Some(setting)) => {
                let raw: String = serde_json::from_str(&setting.value_json).unwrap_or_default();
                raw.split(|c| c == ',' || c == '\n')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_lowercase())
                    .collect()
            }
            _ => Vec::new(),
        };

        SafetySettings {
            block_file_delete,
            block_process_kill,
            block_system_shutdown,
            block_disk_format,
            custom_blocked_keywords,
            enforce_project_folder_only,
        }
    }

    /// Lightweight helper that only reads the `enforceProjectFolderOnly` setting.
    /// Used by dispatch.rs to filter argument-derived path scopes without loading
    /// the full SafetySettings struct a second time.
    pub(crate) async fn load_safety_settings_enforce_only(&self) -> bool {
        self.load_setting_bool("enforceProjectFolderOnly", true).await
    }

    pub(crate) async fn check_safety_filter(
        &self,
        tool: &str,
        _context: &OperationContext,
        arguments: &Value,
        project_folder: Option<&Path>,
    ) -> RuntimeResult<()> {
        let settings = self.load_safety_settings().await;

        // 1. Tool-level check: fs_delete
        if settings.block_file_delete && tool == "fs_delete" {
            return Err(RuntimeError::new(
                "policy_denied",
                "檔案刪除 (fs_delete) 已被系統安全性政策禁止",
            ));
        }

        // 2. Tool-level check: process_kill / shell_signal
        if settings.block_process_kill && matches!(tool, "process_kill" | "shell_signal") {
            return Err(RuntimeError::new(
                "policy_denied",
                "行程終止 (process_kill / shell_signal) 已被系統安全性政策禁止",
            ));
        }

        // 3. Project folder sandbox check
        if settings.enforce_project_folder_only {
            // Tools subject to the sandbox: filesystem ops, shell/command execution, and git.
            // All of these accept path arguments (path, source, destination, cwd, paths[],
            // requests[]) that are checked by check_path_within_project().
            let is_sandboxed_tool = is_filesystem_tool_name(tool)
                || matches!(tool, "command_run" | "shell_create")
                || tool.starts_with("git_");
            if let Some(folder) = project_folder {
                if is_sandboxed_tool {
                    // project_folder is known — verify every path argument is inside it
                    check_path_within_project(arguments, folder)?;
                }
            } else if is_sandboxed_tool {
                // No project_folder is set for this conversation (Unclassified).
                // Without a known folder boundary we cannot enforce the sandbox, so
                // we block filesystem/shell operations entirely to prevent cross-directory access.
                return Err(RuntimeError::new(
                    "policy_denied",
                    "沙箱模式已啟用，但此對話尚未設定專案目錄。\
                    請先將對話移到專案目錄下（右鍵選單 → Move to project），\
                    或在設定中關閉「僅限專案目錄」選項。\
                    (Sandbox is enabled but this conversation has no project folder assigned. \
                    Right-click the conversation → 'Move to project', \
                    or disable the Project-folder-only sandbox in Settings.)",
                ));
            }
        }

        // 4. Command & Shell text check
        let command_strings = extract_command_strings(tool, arguments);
        for cmd_text in command_strings {
            let lower = cmd_text.to_lowercase();

            // A. Check shutdown/reboot
            if settings.block_system_shutdown {
                let shutdown_keywords = [
                    "shutdown",
                    "restart-computer",
                    "stop-computer",
                    "init 0",
                    "init 6",
                    "reboot",
                    "poweroff",
                    "halt",
                ];
                for kw in shutdown_keywords {
                    if contains_command_word(&lower, kw) {
                        return Err(RuntimeError::new(
                            "policy_denied",
                            format!("關機或重新開機相關指令 ('{kw}') 已被系統安全性政策禁止"),
                        ));
                    }
                }
            }

            // B. Check format/diskpart
            if settings.block_disk_format {
                let format_keywords = ["format", "diskpart", "mkfs", "fdisk", "parted"];
                for kw in format_keywords {
                    if contains_command_word(&lower, kw) {
                        return Err(RuntimeError::new(
                            "policy_denied",
                            format!("磁碟格式化或分割指令 ('{kw}') 已被系統安全性政策禁止"),
                        ));
                    }
                }
            }

            // C. Check kill
            if settings.block_process_kill {
                let kill_keywords = [
                    "taskkill",
                    "kill",
                    "pkill",
                    "killall",
                    "stop-process",
                ];
                for kw in kill_keywords {
                    if contains_command_word(&lower, kw) {
                        return Err(RuntimeError::new(
                            "policy_denied",
                            format!("行程終止指令 ('{kw}') 已被系統安全性政策禁止"),
                        ));
                    }
                }
            }

            // D. Check delete
            if settings.block_file_delete {
                let delete_keywords = [
                    "rmdir",
                    "remove-item",
                    "del ",
                    "rm ",
                    "rmdir /s",
                    "del /s",
                    "del /f",
                    "rm -rf",
                    "rm -r",
                ];
                for kw in delete_keywords {
                    if lower.contains(kw) || contains_command_word(&lower, kw.trim()) {
                        return Err(RuntimeError::new(
                            "policy_denied",
                            format!("檔案或目錄刪除指令 ('{kw}') 已被系統安全性政策禁止"),
                        ));
                    }
                }
            }

            // E. Custom blocked keywords
            for custom_kw in &settings.custom_blocked_keywords {
                if !custom_kw.is_empty() && lower.contains(custom_kw) {
                    return Err(RuntimeError::new(
                        "policy_denied",
                        format!("指令包含自訂黑名單關鍵字 ('{custom_kw}')，已被禁止執行"),
                    ));
                }
            }

            // F. Sandbox: scan command text for absolute paths outside project_folder.
            // This blocks shell_write commands like `type C:\Users\frank\secret.txt` or
            // `Get-Content D:\other_project\config.json` that would access files outside
            // the sandbox even when the shell's cwd is inside the project.
            if settings.enforce_project_folder_only {
                if let Some(folder) = project_folder {
                    let canonical_project = folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
                    let canonical_project_lower = canonical_project.to_string_lossy().to_lowercase();
                    // Tokenise the command text and check every token that looks like an absolute path.
                    for token in cmd_text.split_whitespace() {
                        // Strip surrounding quotes if present.
                        let token = token.trim_matches(|c| c == '"' || c == '\'');
                        if looks_like_absolute_path(token) {
                            let p = std::path::Path::new(token);
                            let canonical_p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
                            let canonical_p_lower = canonical_p.to_string_lossy().to_lowercase();
                            if !canonical_p_lower.starts_with(&canonical_project_lower) {
                                return Err(RuntimeError::new(
                                    "policy_denied",
                                    format!(
                                        "指令包含超出限制專案目錄範圍的路徑：{token} (限定於 {})\
                                        \nThe command references a path outside the allowed project directory.",
                                        canonical_project.display()
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

fn contains_command_word(text: &str, word: &str) -> bool {
    let word = word.trim();
    if word.is_empty() {
        return false;
    }
    for token in text.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        if clean.eq_ignore_ascii_case(word) {
            return true;
        }
    }
    if word.contains(' ') || word.contains('/') || word.contains('-') {
        text.contains(word)
    } else {
        false
    }
}

fn extract_command_strings(tool: &str, arguments: &Value) -> Vec<String> {
    let mut result = Vec::new();
    match tool {
        "command_run" => {
            if let Some(exec) = arguments.get("executable").and_then(Value::as_str) {
                result.push(exec.to_owned());
            }
            if let Some(args) = arguments.get("arguments").and_then(Value::as_array) {
                for arg in args {
                    if let Some(s) = arg.as_str() {
                        result.push(s.to_owned());
                    }
                }
            }
        }
        "shell_create" => {
            if let Some(exec) = arguments.get("executable").and_then(Value::as_str) {
                result.push(exec.to_owned());
            }
            if let Some(args) = arguments.get("arguments").and_then(Value::as_array) {
                for arg in args {
                    if let Some(s) = arg.as_str() {
                        result.push(s.to_owned());
                    }
                }
            }
        }
        "shell_write" => {
            if let Some(text) = arguments.get("text").and_then(Value::as_str) {
                result.push(text.to_owned());
            }
        }
        _ => {}
    }
    result
}

fn check_path_within_project(arguments: &Value, project_folder: &Path) -> RuntimeResult<()> {
    let canonical_project = project_folder.canonicalize().unwrap_or_else(|_| project_folder.to_path_buf());
    // Normalise the canonical project path to lowercase for case-insensitive comparison on Windows.
    let canonical_project_lower = canonical_project.to_string_lossy().to_lowercase();

    let check_path = |path_str: &str| -> RuntimeResult<()> {
        let p = Path::new(path_str);
        if p.is_absolute() {
            let canonical_p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            // Use lowercase comparison to handle Windows case-insensitive paths.
            let canonical_p_lower = canonical_p.to_string_lossy().to_lowercase();
            if !canonical_p_lower.starts_with(&canonical_project_lower) {
                return Err(RuntimeError::new(
                    "policy_denied",
                    format!(
                        "路徑存取超出限制專案目錄範圍：{} (限定於 {})",
                        p.display(),
                        canonical_project.display()
                    ),
                ));
            }
        }
        Ok(())
    };

    // Check well-known single-path keys: filesystem tools use path/source/destination/cwd,
    // shell_create uses working_directory, command_run uses executable.
    for key in &["path", "source", "destination", "cwd", "working_directory", "executable"] {
        if let Some(val) = arguments.get(*key).and_then(Value::as_str) {
            check_path(val)?;
        }
    }
    // Check array of paths (fs_list, fs_delete_multi, etc.)
    if let Some(paths) = arguments.get("paths").and_then(Value::as_array) {
        for val in paths {
            if let Some(s) = val.as_str() {
                check_path(s)?;
            }
        }
    }
    // Check requests[].path used by fs_read_text_v2 / fs_batch_read.
    if let Some(requests) = arguments.get("requests").and_then(Value::as_array) {
        for req in requests {
            if let Some(p) = req.get("path").and_then(Value::as_str) {
                check_path(p)?;
            }
        }
    }
    // Deep scan: check ALL string values in the arguments that look like absolute paths.
    // This catches any non-standard argument key the AI might use to smuggle a cross-directory path.
    scan_all_absolute_paths(arguments, &check_path)?;

    Ok(())
}

/// Recursively walks every string value in a JSON tree and calls `check` on
/// any value that looks like an absolute filesystem path (starts with a drive
/// letter on Windows, or `/` on Unix).
fn scan_all_absolute_paths<F>(value: &Value, check: &F) -> RuntimeResult<()>
where
    F: Fn(&str) -> RuntimeResult<()>,
{
    match value {
        Value::String(s) => {
            if looks_like_absolute_path(s) {
                check(s)?;
            }
        }
        Value::Array(arr) => {
            for item in arr {
                scan_all_absolute_paths(item, check)?;
            }
        }
        Value::Object(map) => {
            for (_, v) in map {
                scan_all_absolute_paths(v, check)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Returns true if the string looks like an absolute filesystem path.
/// Matches Windows drive paths (`C:\...`, `D:/...`) and Unix absolute paths (`/...`).
fn looks_like_absolute_path(s: &str) -> bool {
    if s.starts_with('/') {
        return true;
    }
    // Windows drive letter: one letter followed by ':' and '\' or '/'
    if s.len() >= 3 {
        let bytes = s.as_bytes();
        if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
            return true;
        }
    }
    false
}
