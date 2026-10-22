use std::path::PathBuf;

#[path = "safety_filter_paths.rs"]
mod paths;
use chatcmd_core::SettingsStore as _;
use chatcmd_runtime::{OperationContext, RuntimeError, RuntimeResult};
use paths::{
    check_path_within_projects, command_tokens, looks_like_absolute_path,
    validate_path_within_projects,
};
pub(crate) use paths::{is_subpath, strip_verbatim_prefix};
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
            Ok(Some(setting)) => {
                serde_json::from_str::<bool>(&setting.value_json).unwrap_or(default)
            }
            _ => default,
        }
    }

    pub(crate) async fn load_safety_settings(&self) -> SafetySettings {
        let block_file_delete = self.load_setting_bool("blockFileDelete", true).await;
        let block_process_kill = self.load_setting_bool("blockProcessKill", true).await;
        let block_system_shutdown = self.load_setting_bool("blockSystemShutdown", true).await;
        let block_disk_format = self.load_setting_bool("blockDiskFormat", true).await;
        let enforce_project_folder_only = self
            .load_setting_bool("enforceProjectFolderOnly", true)
            .await;

        let custom_blocked_keywords =
            match self.repository.setting("ui_customBlockedKeywords").await {
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
        self.load_setting_bool("enforceProjectFolderOnly", true)
            .await
    }

    pub(crate) async fn check_safety_filter(
        &self,
        tool: &str,
        _context: &OperationContext,
        arguments: &Value,
        project_folders: &[PathBuf],
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
            // requests[]) that are checked by check_path_within_projects().
            let is_sandboxed_tool = is_filesystem_tool_name(tool)
                || matches!(tool, "command_run" | "shell_create")
                || tool.starts_with("git_");
            if !project_folders.is_empty() {
                if is_sandboxed_tool {
                    // project_folders is known — verify every path argument is inside at least one folder
                    check_path_within_projects(arguments, project_folders)?;
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
                let kill_keywords = ["taskkill", "kill", "pkill", "killall", "stop-process"];
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
                    "del",
                    "erase",
                    "rm",
                ];
                for kw in delete_keywords {
                    // Match the executable token, not a raw substring: a path such as
                    // `C:\model FILES` contains the characters `del ` but is not a delete command.
                    if contains_command_word(&lower, kw) {
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

            // F. Sandbox: scan command text for absolute paths outside project_folders.
            // This blocks shell_write commands like `type C:\Users\frank\secret.txt` or
            // `Get-Content D:\other_project\config.json` that would access files outside
            // the sandbox even when the shell's cwd is inside the project.
            if settings.enforce_project_folder_only && !project_folders.is_empty() {
                let canonical_projects: Vec<PathBuf> = project_folders
                    .iter()
                    .map(|f| strip_verbatim_prefix(&f.canonicalize().unwrap_or_else(|_| f.clone())))
                    .collect();
                // Tokenise the command text and check every token that looks like an absolute path.
                for token in command_tokens(&cmd_text) {
                    // Strip surrounding quotes if present.
                    let token = token.trim_matches(|c| c == '"' || c == '\'');
                    // After quote-stripping, also strip trailing shell punctuation that may
                    // appear after a closing quote but before whitespace (e.g. 'D:\path'; or
                    // 'D:\path'| or 'D:\path'&&).  These characters are not part of the path.
                    let token = token.trim_end_matches(|c: char| {
                        matches!(c, ';' | '|' | '&' | ',' | ')' | '(' | '`' | '\'' | '"')
                    });
                    if looks_like_absolute_path(token) {
                        let p = std::path::Path::new(token);
                        if validate_path_within_projects(p, &canonical_projects).is_err() {
                            let allowed_display: Vec<String> = canonical_projects
                                .iter()
                                .map(|cp| cp.display().to_string())
                                .collect();
                            return Err(RuntimeError::new(
                                "policy_denied",
                                format!(
                                    "指令包含超出限制專案目錄範圍的路徑：{token} (限定於 {})\
                                    \nThe command references a path outside the allowed project directory.",
                                    allowed_display.join(", ")
                                ),
                            ));
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

#[cfg(test)]
#[path = "safety_filter_tests.rs"]
mod tests;
