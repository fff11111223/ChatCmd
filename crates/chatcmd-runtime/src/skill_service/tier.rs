use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const MAX_RESIDENT_CHARS_PER_SKILL: usize = 300;
pub const MAX_RESIDENT_CHARS_TOTAL: usize = 800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSourceType {
    /// Discovered from legacy `.agents/skills` or `.codex/skills` roots
    LegacyAgentOrCodex,
    /// Discovered from global skills root
    Global,
    /// Project-specific skill under `skills/projects/<project_name>/`
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LintSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillLintDiagnostic {
    pub severity: LintSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SkillTierInfo {
    pub has_resident: bool,
    pub resident_content: Option<String>,
    pub has_core: bool,
    pub core_path: Option<PathBuf>,
    pub examples: Vec<String>,
    pub overrides: Option<String>,
}

/// Inspect the 3-tier structure of a skill directory and return tier information along with diagnostics.
pub fn inspect_skill_tier(
    directory: &Path,
    source_type: SkillSourceType,
) -> (SkillTierInfo, Vec<SkillLintDiagnostic>) {
    let mut info = SkillTierInfo::default();
    let mut diagnostics = Vec::new();

    // 1. Inspect resident.md
    let resident_file = directory.join("resident.md");
    if resident_file.is_file() {
        info.has_resident = true;
        match fs::read_to_string(&resident_file) {
            Ok(content) => {
                let trimmed = content.trim();
                let char_count = trimmed.chars().count();
                if source_type == SkillSourceType::LegacyAgentOrCodex {
                    diagnostics.push(SkillLintDiagnostic {
                        severity: LintSeverity::Warning,
                        code: "legacy_resident_ignored".into(),
                        message: "Skill from legacy .agents/.codex source contains resident.md which is ignored.".into(),
                        path: Some(resident_file.to_string_lossy().into_owned()),
                    });
                    info.resident_content = None;
                } else {
                    if char_count > MAX_RESIDENT_CHARS_PER_SKILL {
                        diagnostics.push(SkillLintDiagnostic {
                            severity: LintSeverity::Error,
                            code: "resident_too_long".into(),
                            message: format!(
                                "resident.md exceeds limit of {} characters (actual: {} characters).",
                                MAX_RESIDENT_CHARS_PER_SKILL, char_count
                            ),
                            path: Some(resident_file.to_string_lossy().into_owned()),
                        });
                    }
                    info.resident_content = Some(trimmed.to_owned());
                }
            }
            Err(error) => {
                diagnostics.push(SkillLintDiagnostic {
                    severity: LintSeverity::Error,
                    code: "resident_read_failed".into(),
                    message: format!("Could not read resident.md: {error}"),
                    path: Some(resident_file.to_string_lossy().into_owned()),
                });
            }
        }
    }

    // 2. Inspect core.md vs legacy SKILL.md
    let core_file = directory.join("core.md");
    let legacy_file = directory.join("SKILL.md");

    if core_file.is_file() {
        info.has_core = true;
        info.core_path = Some(core_file);
    } else if legacy_file.is_file() {
        info.has_core = true;
        info.core_path = Some(legacy_file.clone());
        diagnostics.push(SkillLintDiagnostic {
            severity: LintSeverity::Warning,
            code: "legacy_skill_format".into(),
            message: "Skill is using legacy SKILL.md format without core.md.".into(),
            path: Some(legacy_file.to_string_lossy().into_owned()),
        });
    } else {
        diagnostics.push(SkillLintDiagnostic {
            severity: LintSeverity::Error,
            code: "missing_core".into(),
            message: "Skill directory must contain core.md or SKILL.md.".into(),
            path: Some(directory.to_string_lossy().into_owned()),
        });
    }

    // Parse frontmatter if core/SKILL.md exists
    if let Some(target) = &info.core_path {
        if let Ok(content) = fs::read_to_string(target) {
            let frontmatter = parse_frontmatter_tier(&content);
            if let Some(overrides_target) = frontmatter.get("overrides").cloned() {
                let trimmed = overrides_target.trim();
                if !trimmed.is_empty() {
                    info.overrides = Some(trimmed.to_owned());
                }
            }
        }
    }

    // 3. Inspect examples/<tool_key>.md
    let examples_dir = directory.join("examples");
    if examples_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&examples_dir) {
            let mut found_examples = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or_default();
                    if let Some(stem) = name.strip_suffix(".md") {
                        if !stem.is_empty() {
                            found_examples.push(stem.to_owned());
                        }
                    } else if !name.starts_with('.') {
                        found_examples.push(name.to_owned());
                    }
                }
            }
            found_examples.sort();
            info.examples = found_examples;
        }
    }

    (info, diagnostics)
}

/// Merge resident instructions from multiple skills, strictly enforcing the total character budget.
/// Returns (merged_text, warnings).
pub fn merge_resident_instructions(
    active_residents: &[(&str, &str)], // (skill_name, content)
) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    if active_residents.is_empty() {
        return (String::new(), warnings);
    }

    let mut blocks = Vec::with_capacity(active_residents.len());

    for (name, content) in active_residents {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            continue;
        }
        blocks.push(format!("### Skill: {name}\n{trimmed}"));
    }

    let joined = blocks.join("\n\n");
    let joined_chars = joined.chars().count();

    if joined_chars > MAX_RESIDENT_CHARS_TOTAL {
        warnings.push(format!(
            "Total resident instructions exceeded limit of {} characters (actual: {} characters); truncated to limit.",
            MAX_RESIDENT_CHARS_TOTAL, joined_chars
        ));
        let truncated: String = joined.chars().take(MAX_RESIDENT_CHARS_TOTAL).collect();
        (truncated, warnings)
    } else {
        (joined, warnings)
    }
}

/// Helper to parse frontmatter from markdown.
fn parse_frontmatter_tier(content: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return values;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some((key, val)) = trimmed.split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            let val = val.trim().trim_matches(['"', '\'']);
            values.insert(key, val.to_owned());
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_3_tier_structure() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join("my-skill");
        fs::create_dir_all(skill_dir.join("examples")).expect("create dirs");

        fs::write(
            skill_dir.join("resident.md"),
            "Use strict typing when writing functions.",
        )
        .expect("write resident");
        fs::write(
            skill_dir.join("core.md"),
            "---\nname: my-skill\ndescription: Test skill\noverrides: global-skill\n---\nCore instructions here.",
        )
        .expect("write core");
        fs::write(
            skill_dir.join("examples").join("command_run.md"),
            "command_run example content",
        )
        .expect("write example");

        let (info, diagnostics) = inspect_skill_tier(&skill_dir, SkillSourceType::Project);

        assert!(info.has_resident);
        assert_eq!(
            info.resident_content.as_deref(),
            Some("Use strict typing when writing functions.")
        );
        assert!(info.has_core);
        assert_eq!(info.overrides.as_deref(), Some("global-skill"));
        assert_eq!(info.examples, vec!["command_run"]);
        assert!(diagnostics.is_empty(), "unexpected diagnostics: {diagnostics:?}");
    }

    #[test]
    fn resident_too_long_reports_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join("long-resident");
        fs::create_dir_all(&skill_dir).expect("create dir");

        let long_text = "a".repeat(301);
        fs::write(skill_dir.join("resident.md"), &long_text).expect("write resident");
        fs::write(skill_dir.join("core.md"), "Core").expect("write core");

        let (info, diagnostics) = inspect_skill_tier(&skill_dir, SkillSourceType::Global);

        assert!(info.has_resident);
        assert!(diagnostics.iter().any(|d| d.severity == LintSeverity::Error && d.code == "resident_too_long"));
    }

    #[test]
    fn legacy_resident_ignored_with_warning() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join("legacy-skill");
        fs::create_dir_all(&skill_dir).expect("create dir");

        fs::write(skill_dir.join("resident.md"), "Resident content").expect("write resident");
        fs::write(skill_dir.join("SKILL.md"), "Legacy skill").expect("write SKILL.md");

        let (info, diagnostics) = inspect_skill_tier(&skill_dir, SkillSourceType::LegacyAgentOrCodex);

        assert!(info.has_resident);
        assert_eq!(info.resident_content, None); // Ignored!
        assert!(diagnostics.iter().any(|d| d.severity == LintSeverity::Warning && d.code == "legacy_resident_ignored"));
        assert!(diagnostics.iter().any(|d| d.severity == LintSeverity::Warning && d.code == "legacy_skill_format"));
    }

    #[test]
    fn merge_resident_truncates_at_limit() {
        let r1 = "a".repeat(250);
        let r2 = "b".repeat(250);
        let r3 = "c".repeat(250);
        let r4 = "d".repeat(250);

        let (merged, warnings) = merge_resident_instructions(&[
            ("s1", &r1),
            ("s2", &r2),
            ("s3", &r3),
            ("s4", &r4),
        ]);

        assert_eq!(merged.chars().count(), MAX_RESIDENT_CHARS_TOTAL);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("truncated"));
    }
}
