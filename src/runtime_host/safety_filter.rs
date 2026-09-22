use std::path::Path;
use chatcmd_core::SettingsStore as _;
use chatcmd_runtime::{OperationContext, RuntimeError, RuntimeResult};
use serde_json::Value;

use super::RuntimeHost;

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
            if let Some(folder) = project_folder {
                if let Err(err) = check_path_within_project(arguments, folder) {
                    return Err(err);
                }
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

    let check_path = |path_str: &str| -> RuntimeResult<()> {
        let p = Path::new(path_str);
        if p.is_absolute() {
            let canonical_p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            if !canonical_p.starts_with(&canonical_project) {
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

    for key in &["path", "source", "destination", "cwd"] {
        if let Some(val) = arguments.get(*key).and_then(Value::as_str) {
            check_path(val)?;
        }
    }
    if let Some(paths) = arguments.get("paths").and_then(Value::as_array) {
        for val in paths {
            if let Some(s) = val.as_str() {
                check_path(s)?;
            }
        }
    }

    Ok(())
}
