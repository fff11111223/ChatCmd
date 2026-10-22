use super::paths::*;
use super::*;
use std::fs;
use std::path::Path;
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
    let escape_via_parent = project_dir
        .join("..")
        .join("other_folder")
        .join("secret.txt");
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
                assert!(
                    validate_path_within_project(&file_via_symlink, &canonical_project).is_err()
                );
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
    assert!(check_path_within_projects(&args, &[project_dir.clone()]).is_ok());

    // Sibling folder ChatCmd2 rejected
    let sibling_file = temp.path().join("ChatCmd2").join("secret.txt");
    let args_sibling = serde_json::json!({
        "path": sibling_file.to_string_lossy().to_string(),
    });
    assert!(check_path_within_projects(&args_sibling, &[project_dir]).is_err());
}

#[test]
fn test_check_paths_within_multiple_projects() {
    let temp = tempdir().expect("tempdir");
    let project_dir1 = temp.path().join("ChatCmd1");
    let project_dir2 = temp.path().join("ChatCmd2");
    let outside_dir = temp.path().join("ChatCmdOutside");
    fs::create_dir_all(&project_dir1).expect("create dir1");
    fs::create_dir_all(&project_dir2).expect("create dir2");
    fs::create_dir_all(&outside_dir).expect("create outside");

    let file1 = project_dir1.join("file1.txt");
    let file2 = project_dir2.join("file2.txt");
    let outside_file = outside_dir.join("secret.txt");
    fs::write(&file1, "content1").expect("write file1");
    fs::write(&file2, "content2").expect("write file2");
    fs::write(&outside_file, "secret").expect("write outside");

    let folders = vec![project_dir1.clone(), project_dir2.clone()];

    // Both folder 1 and folder 2 files are allowed
    let args1 = serde_json::json!({ "path": file1.to_string_lossy().to_string() });
    let args2 = serde_json::json!({ "path": file2.to_string_lossy().to_string() });
    assert!(check_path_within_projects(&args1, &folders).is_ok());
    assert!(check_path_within_projects(&args2, &folders).is_ok());

    // File outside both folders is rejected
    let args_outside = serde_json::json!({ "path": outside_file.to_string_lossy().to_string() });
    assert!(check_path_within_projects(&args_outside, &folders).is_err());
}

/// Regression test: a quoted PowerShell path followed immediately by ';' (e.g.
/// `Set-Location 'D:\frank\REA'; Write-Output ...`) was previously extracted
/// as the token `'D:\frank\REA';`, and after quote-stripping the trailing `;`
/// remained, producing `D:\frank\REA';` which failed the path-within-project
/// check even though the path was valid.
#[test]
fn test_command_tokens_quoted_path_with_trailing_semicolon() {
    use std::path::Path;

    let temp = tempdir().expect("tempdir");
    let project_dir = temp.path().join("REA");
    fs::create_dir_all(&project_dir).expect("create project dir");

    let canonical_project = strip_verbatim_prefix(
        &project_dir
            .canonicalize()
            .unwrap_or_else(|_| project_dir.to_path_buf()),
    );
    let canonical_projects = vec![canonical_project.clone()];

    let project_str = canonical_project.to_string_lossy().to_string();

    // Simulate the exact pattern from the bug report:
    // Set-Location 'D:\frank\REA'; Write-Output '=== ZIP FILE ==='
    let cmd = format!("Set-Location '{project_str}'; Write-Output '=== ZIP FILE ==='");

    for token in command_tokens(&cmd) {
        // Strip surrounding quotes, then strip trailing shell punctuation — same as safety_filter.rs
        let token = token.trim_matches(|c| c == '"' || c == '\'');
        let token = token.trim_end_matches(|c: char| {
            matches!(c, ';' | '|' | '&' | ',' | ')' | '(' | '`' | '\'' | '"')
        });
        if looks_like_absolute_path(token) {
            let p = Path::new(token);
            assert!(
                validate_path_within_projects(p, &canonical_projects).is_ok(),
                "Path '{token}' should be within project '{project_str}'"
            );
        }
    }
}
