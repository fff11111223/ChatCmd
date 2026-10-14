use std::path::{Path, PathBuf};

use chatcmd_runtime::{RuntimeError, RuntimeResult};
use serde_json::Value;

pub(super) fn check_path_within_projects(
    arguments: &Value,
    project_folders: &[PathBuf],
) -> RuntimeResult<()> {
    let canonical_projects: Vec<PathBuf> = project_folders
        .iter()
        .map(|f| strip_verbatim_prefix(&f.canonicalize().unwrap_or_else(|_| f.clone())))
        .collect();

    let primary = &project_folders[0];
    let check_path = |path_str: &str| -> RuntimeResult<()> {
        let p = Path::new(path_str);
        let abs_path = if looks_like_absolute_path(path_str) || p.is_absolute() {
            p.to_path_buf()
        } else {
            primary.join(p)
        };
        validate_path_within_projects(&abs_path, &canonical_projects)?;
        Ok(())
    };

    // Check well-known single-path keys: filesystem tools use path/source/destination/cwd,
    // shell_create uses working_directory, command_run uses executable.
    for key in &[
        "path",
        "source",
        "destination",
        "cwd",
        "working_directory",
        "executable",
    ] {
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
pub(super) fn looks_like_absolute_path(s: &str) -> bool {
    if s.starts_with('/') || s.starts_with(r"\\") || s.starts_with("//") {
        return true;
    }
    // Windows drive letter: one letter followed by ':' and '\' or '/'
    if s.len() >= 3 {
        let bytes = s.as_bytes();
        if bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/')
        {
            return true;
        }
    }
    false
}

/// Strips Windows extended-length prefix (`\\?\` or `//?/`) from a path.
pub(crate) fn strip_verbatim_prefix(path: &Path) -> PathBuf {
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
        if rest.len() < 2 || !rest.as_bytes()[0].is_ascii_alphabetic() || rest.as_bytes()[1] != b':'
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
    if clean.len() >= 2 && clean.as_bytes()[0].is_ascii_alphabetic() && clean.as_bytes()[1] == b':'
    {
        clean[2..].contains(':')
    } else {
        clean.contains(':')
    }
}

/// Checks whether `target` is within `base` (or equal to `base`) by comparing path components.
/// On Windows, component names are compared case-insensitively.
pub(crate) fn is_subpath(target: &Path, base: &Path) -> bool {
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

/// Validates that `target` is within at least one of `canonical_projects`.
pub(super) fn validate_path_within_projects(
    target: &Path,
    canonical_projects: &[PathBuf],
) -> RuntimeResult<()> {
    let raw_str = target.to_string_lossy();
    let display_str = canonical_projects
        .iter()
        .map(|cp| cp.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    if is_unc_or_device_path(&raw_str) {
        return Err(RuntimeError::new(
            "policy_denied",
            format!(
                "路徑存取超出限制專案目錄範圍：{} (限定於 {})",
                target.display(),
                display_str
            ),
        ));
    }
    if has_alternate_data_stream(&raw_str) {
        return Err(RuntimeError::new(
            "policy_denied",
            format!(
                "路徑存取超出限制專案目錄範圍：{} (限定於 {})",
                target.display(),
                display_str
            ),
        ));
    }

    let resolved_target = resolve_target_canonical(target)?;

    let is_inside_any = canonical_projects
        .iter()
        .any(|canonical_project| is_subpath(&resolved_target, canonical_project));

    if !is_inside_any {
        return Err(RuntimeError::new(
            "policy_denied",
            format!(
                "路徑存取超出限制專案目錄範圍：{} (限定於 {})",
                target.display(),
                display_str
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
pub(super) fn validate_path_within_project(
    target: &Path,
    canonical_project: &Path,
) -> RuntimeResult<()> {
    validate_path_within_projects(target, &[canonical_project.to_path_buf()])
}

/// Splits shell text on whitespace outside quoted strings.
/// If quoting is malformed, fall back to whitespace tokens so the safety scan
/// remains conservative instead of silently skipping later absolute paths.
pub(super) fn command_tokens(command: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut token_start = None;
    let mut quote = None;
    let mut chars = command.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                if active_quote == '\'' && chars.peek().is_some_and(|(_, next)| *next == '\'') {
                    chars.next();
                } else {
                    quote = None;
                }
            } else if active_quote == '"' && ch == '`' {
                chars.next();
            } else if active_quote == '"'
                && ch == '\\'
                && chars.peek().is_some_and(|(_, next)| *next == '"')
            {
                chars.next();
            }
            continue;
        }

        if ch.is_whitespace() {
            if let Some(start) = token_start.take() {
                tokens.push(&command[start..index]);
            }
        } else {
            token_start.get_or_insert(index);
            if ch == '\'' || ch == '"' {
                quote = Some(ch);
            }
        }
    }

    if quote.is_some() {
        return command.split_whitespace().collect();
    }
    if let Some(start) = token_start {
        tokens.push(&command[start..]);
    }
    tokens
}
