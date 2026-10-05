use std::path::{Path, PathBuf};
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
                    let canonical_project = strip_verbatim_prefix(
                        &folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf()),
                    );
                    // Tokenise the command text and check every token that looks like an absolute path.
                    for token in cmd_text.split_whitespace() {
                        // Strip surrounding quotes if present.
                        let token = token.trim_matches(|c| c == '"' || c == '\'');
                        if looks_like_absolute_path(token) {
                            let p = std::path::Path::new(token);
                            if validate_path_within_project(p, &canonical_project).is_err() {
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
    let canonical_project = strip_verbatim_prefix(
        &project_folder
            .canonicalize()
            .unwrap_or_else(|_| project_folder.to_path_buf()),
    );

    let check_path = |path_str: &str| -> RuntimeResult<()> {
        let p = Path::new(path_str);
        let abs_path = if looks_like_absolute_path(path_str) || p.is_absolute() {
            p.to_path_buf()
        } else {
            project_folder.join(p)
        };
        validate_path_within_project(&abs_path, &canonical_project)?;
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
/// any value that looks like an absolute filesystem path.
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
/// Matches Windows drive paths (`C:\...`, `D:/...`), UNC paths (`\\...`, `//...`),
/// and Unix absolute paths (`/...`).
fn looks_like_absolute_path(s: &str) -> bool {
    if s.starts_with('/') || s.starts_with(r"\\") || s.starts_with("//") {
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

/// Strips Windows extended-length prefix (`\\?\` or `//?/`) from a path.
fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else if let Some(stripped) = s.strip_prefix("//?/") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

/// Returns true if the path string is a network UNC path or Windows device namespace.
fn is_unc_or_device_path(s: &str) -> bool {
    // 1. Verbatim UNC: \\?\UNC\ or //?/UNC/
    if s.starts_with(r"\\?\UNC\")
        || s.starts_with(r"\\?\unc\")
        || s.starts_with(r"//?/UNC/")
        || s.starts_with(r"//?/unc/")
    {
        return true;
    }
    // 2. Device namespace: \\.\ or //./
    if s.starts_with(r"\\.\") || s.starts_with(r"//./") {
        return true;
    }
    // 3. General UNC: \\server\share or //server/share (not verbatim \\?\)
    if (s.starts_with(r"\\") && !s.starts_with(r"\\?\"))
        || (s.starts_with("//") && !s.starts_with("//?/"))
    {
        return true;
    }
    // 4. Verbatim path that is not a drive letter: e.g. \\?\Volume{...}
    if s.starts_with(r"\\?\") || s.starts_with(r"//?/") {
        let rest = &s[4..];
        if rest.len() < 2
            || !rest.as_bytes()[0].is_ascii_alphabetic()
            || rest.as_bytes()[1] != b':'
        {
            return true;
        }
    }
    false
}

/// Returns true if the path contains an NTFS Alternate Data Stream specification (`:stream`).
fn has_alternate_data_stream(s: &str) -> bool {
    let clean = if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped
    } else if let Some(stripped) = s.strip_prefix(r"//?/") {
        stripped
    } else {
        s
    };
    // If it starts with a drive letter e.g. "C:", skip the first 2 characters.
    if clean.len() >= 2
        && clean.as_bytes()[0].is_ascii_alphabetic()
        && clean.as_bytes()[1] == b':'
    {
        clean[2..].contains(':')
    } else {
        clean.contains(':')
    }
}

/// Checks whether `target` is within `base` (or equal to `base`) by comparing path components.
/// On Windows, component names are compared case-insensitively.
fn is_subpath(target: &Path, base: &Path) -> bool {
    let mut target_comps = target
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir));
    let mut base_comps = base
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir));

    loop {
        match (base_comps.next(), target_comps.next()) {
            (None, _) => {
                // All components of base matched. Target is inside base.
                return true;
            }
            (Some(b), Some(t)) => {
                let matches = match (b, t) {
                    (std::path::Component::Prefix(bp), std::path::Component::Prefix(tp)) => {
                        bp.as_os_str().eq_ignore_ascii_case(tp.as_os_str())
                    }
                    (std::path::Component::RootDir, std::path::Component::RootDir) => true,
                    (std::path::Component::Normal(bn), std::path::Component::Normal(tn)) => {
                        if cfg!(windows) {
                            bn.eq_ignore_ascii_case(tn)
                        } else {
                            bn == tn
                        }
                    }
                    _ => false,
                };
                if !matches {
                    return false;
                }
            }
            (Some(_), None) => {
                return false;
            }
        }
    }
}

/// Resolves a target path to a canonical form:
/// - If the target exists, canonicalizes it directly (resolving symlinks and junctions).
/// - If the target does not exist, walks up to the nearest existing ancestor, canonicalizes it,
///   and appends the remaining non-existent components.
fn resolve_target_canonical(target_path: &Path) -> RuntimeResult<PathBuf> {
    if target_path.exists() {
        let canonical = target_path.canonicalize().map_err(|e| {
            RuntimeError::new(
                "policy_denied",
                format!("無法解析路徑 {}：{e}", target_path.display()),
            )
        })?;
        return Ok(strip_verbatim_prefix(&canonical));
    }

    // Target does not exist (e.g. writing a new file or directory).
    // Walk up to find the nearest existing ancestor.
    let mut current = target_path.to_path_buf();
    let mut tail_components = Vec::new();

    loop {
        // If current is a symlink (including broken symlinks pointing outside), follow it.
        if let Ok(meta) = std::fs::symlink_metadata(&current) {
            if meta.is_symlink() {
                if let Ok(link_target) = std::fs::read_link(&current) {
                    let resolved_link = if link_target.is_absolute() {
                        link_target
                    } else if let Some(parent) = current.parent() {
                        parent.join(link_target)
                    } else {
                        link_target
                    };
                    let mut full_target = resolved_link;
                    for comp in tail_components.iter().rev() {
                        full_target.push(comp);
                    }
                    return resolve_target_canonical(&full_target);
                }
            }
        }

        if current.exists() {
            break;
        }
        if let Some(file_name) = current.file_name() {
            tail_components.push(file_name.to_os_string());
            if !current.pop() {
                break;
            }
        } else {
            break;
        }
    }

    if !current.exists() {
        return Err(RuntimeError::new(
            "policy_denied",
            format!("路徑上層資料夾不存在：{}", target_path.display()),
        ));
    }

    // Canonicalize the existing ancestor (resolves symlinks, junctions, and `..`).
    let canonical_ancestor = current.canonicalize().map_err(|e| {
        RuntimeError::new(
            "policy_denied",
            format!("無法解析上層目錄 {}：{e}", current.display()),
        )
    })?;
    let mut resolved = strip_verbatim_prefix(&canonical_ancestor);

    // Append non-existent tail components in forward order.
    tail_components.reverse();
    for comp in tail_components {
        let comp_path = Path::new(&comp);
        for c in comp_path.components() {
            match c {
                std::path::Component::Normal(n) => {
                    resolved.push(n);
                }
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    return Err(RuntimeError::new(
                        "policy_denied",
                        format!("路徑包含無效的相對跳出元件 (..)：{}", target_path.display()),
                    ));
                }
                _ => {}
            }
        }
    }

    Ok(resolved)
}

/// Validates that `target` is within `canonical_project`.
fn validate_path_within_project(target: &Path, canonical_project: &Path) -> RuntimeResult<()> {
    let raw_str = target.to_string_lossy();
    if is_unc_or_device_path(&raw_str) {
        return Err(RuntimeError::new(
            "policy_denied",
            format!(
                "路徑存取超出限制專案目錄範圍：{} (限定於 {})",
                target.display(),
                canonical_project.display()
            ),
        ));
    }
    if has_alternate_data_stream(&raw_str) {
        return Err(RuntimeError::new(
            "policy_denied",
            format!(
                "路徑存取超出限制專案目錄範圍：{} (限定於 {})",
                target.display(),
                canonical_project.display()
            ),
        ));
    }

    let resolved_target = resolve_target_canonical(target)?;

    if !is_subpath(&resolved_target, canonical_project) {
        return Err(RuntimeError::new(
            "policy_denied",
            format!(
                "路徑存取超出限制專案目錄範圍：{} (限定於 {})",
                target.display(),
                canonical_project.display()
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_sandbox_path_validation() {
        let temp = tempdir().expect("tempdir");
        let project_dir = temp.path().join("ChatCmd");
        fs::create_dir_all(&project_dir).expect("create project dir");

        let canonical_project = strip_verbatim_prefix(
            &project_dir
                .canonicalize()
                .unwrap_or_else(|_| project_dir.to_path_buf()),
        );

        // (a) 範圍內已存在檔案 → 允許
        let existing_file = project_dir.join("existing.txt");
        fs::write(&existing_file, "hello").expect("write file");
        assert!(validate_path_within_project(&existing_file, &canonical_project).is_ok());

        // (a) 範圍內新檔案且上層資料夾不存在 → 允許
        let deeply_nested_new_file = project_dir.join("sub1").join("sub2").join("new_file.md");
        assert!(!deeply_nested_new_file.parent().unwrap().exists());
        assert!(validate_path_within_project(&deeply_nested_new_file, &canonical_project).is_ok());

        // (b) 範圍外路徑 → 全部拒絕
        let outside_dir = temp.path().join("other_folder");
        fs::create_dir_all(&outside_dir).expect("create outside dir");
        let outside_file = outside_dir.join("secret.txt");
        assert!(validate_path_within_project(&outside_file, &canonical_project).is_err());

        // (b) 含 .. 跳出 → 全部拒絕
        let escape_via_parent = project_dir.join("..").join("other_folder").join("secret.txt");
        assert!(validate_path_within_project(&escape_via_parent, &canonical_project).is_err());

        // (b) 同名前綴的兄弟資料夾（ChatCmd2） → 全部拒絕
        let sibling_dir = temp.path().join("ChatCmd2");
        fs::create_dir_all(&sibling_dir).expect("create sibling dir");
        let sibling_file = sibling_dir.join("file.txt");
        assert!(validate_path_within_project(&sibling_file, &canonical_project).is_err());

        // (b) UNC 路徑 → 全部拒絕
        let unc_path = Path::new(r"\\server\share\file.txt");
        assert!(validate_path_within_project(unc_path, &canonical_project).is_err());
        let verbatim_unc = Path::new(r"\\?\UNC\server\share\file.txt");
        assert!(validate_path_within_project(verbatim_unc, &canonical_project).is_err());

        // (b) 替代資料流 (Alternate Data Streams) → 全部拒絕
        let ads_path = project_dir.join("file.txt:stream");
        assert!(validate_path_within_project(&ads_path, &canonical_project).is_err());

        // (b) 符號連結跳出 → 全部拒絕
        #[cfg(windows)]
        {
            let symlink_path = project_dir.join("symlink_outside");
            match std::os::windows::fs::symlink_dir(&outside_dir, &symlink_path) {
                Ok(_) => {
                    let file_via_symlink = symlink_path.join("secret.txt");
                    assert!(validate_path_within_project(&file_via_symlink, &canonical_project).is_err());
                }
                Err(e) if e.raw_os_error() == Some(1314) => {
                    // Windows user without SeCreateSymbolicLinkPrivilege
                }
                Err(e) => panic!("unexpected symlink error: {e}"),
            }
        }
        #[cfg(unix)]
        {
            let symlink_path = project_dir.join("symlink_outside");
            std::os::unix::fs::symlink(&outside_dir, &symlink_path).expect("symlink");
            let file_via_symlink = symlink_path.join("secret.txt");
            assert!(validate_path_within_project(&file_via_symlink, &canonical_project).is_err());
        }

        // (c) 有 \\?\ 前綴與沒有前綴的同一路徑 → 結果一致
        let raw_existing_str = existing_file.to_string_lossy();
        if !raw_existing_str.starts_with(r"\\?\") {
            let verbatim_existing = PathBuf::from(format!(r"\\?\{raw_existing_str}"));
            assert_eq!(
                validate_path_within_project(&existing_file, &canonical_project).is_ok(),
                validate_path_within_project(&verbatim_existing, &canonical_project).is_ok()
            );

            let raw_deep_str = deeply_nested_new_file.to_string_lossy();
            let verbatim_deep = PathBuf::from(format!(r"\\?\{raw_deep_str}"));
            assert_eq!(
                validate_path_within_project(&deeply_nested_new_file, &canonical_project).is_ok(),
                validate_path_within_project(&verbatim_deep, &canonical_project).is_ok()
            );

            let raw_sibling_str = sibling_file.to_string_lossy();
            let verbatim_sibling = PathBuf::from(format!(r"\\?\{raw_sibling_str}"));
            assert_eq!(
                validate_path_within_project(&sibling_file, &canonical_project).is_ok(),
                validate_path_within_project(&verbatim_sibling, &canonical_project).is_ok()
            );
        }
    }

    #[test]
    fn test_check_path_within_project_arguments() {
        let temp = tempdir().expect("tempdir");
        let project_dir = temp.path().join("ChatCmd");
        fs::create_dir_all(&project_dir).expect("create project dir");

        // Write new file in not-yet-created subdirectory (the exact user bug)
        let new_file = project_dir
            .join("target")
            .join("release")
            .join("skills")
            .join("global")
            .join("chatcmd-tools")
            .join("examples")
            .join("device_list.md");
        let args = serde_json::json!({
            "path": new_file.to_string_lossy().to_string(),
            "content": "example content"
        });
        assert!(check_path_within_project(&args, &project_dir).is_ok());

        // Sibling folder ChatCmd2 rejected
        let sibling_file = temp.path().join("ChatCmd2").join("secret.txt");
        let args_sibling = serde_json::json!({
            "path": sibling_file.to_string_lossy().to_string(),
        });
        assert!(check_path_within_project(&args_sibling, &project_dir).is_err());
    }
}

