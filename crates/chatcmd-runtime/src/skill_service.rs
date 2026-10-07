use crate::{RuntimeError, RuntimeResult, SkillReadResult, SkillSummary};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use tokio::process::Command;

mod support;
use support::*;

mod tier;
pub use tier::*;

use std::sync::Arc;
use tokio::sync::RwLock;

const MAX_SKILL_BYTES: u64 = 2_000_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOptionChoice {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOption {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub option_type: String,
    pub value: Value,
    pub choices: Option<Vec<SkillOptionChoice>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSkill {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub icon_path: Option<String>,
    pub source: String,
    pub source_url: Option<String>,
    pub enabled: bool,
    pub can_delete: bool,
    pub options: Vec<SkillOption>,
    // Extended fields (Task 2) — have defaults so old front-end calls are unaffected.
    /// Human-readable source category: "global", "project", "local", "workspace", "user_home".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    /// Whether a resident.md that is actively used exists for this skill.
    #[serde(default)]
    pub has_resident: bool,
    /// Character count of the resident.md content, if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resident_chars: Option<usize>,
    /// Whether core.md (or legacy SKILL.md) exists.
    #[serde(default)]
    pub has_core: bool,
    /// List of tool example keys (files in examples/ without .md extension).
    #[serde(default)]
    pub examples: Vec<String>,
    /// The global skill name that this project skill overrides, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<String>,
    /// Non-fatal warnings from lint or loading.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Errors from lint or loading.
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstallCandidate {
    pub name: String,
    pub title: String,
    pub description: String,
    pub path: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstallPreview {
    pub repository_url: String,
    pub skills: Vec<SkillInstallCandidate>,
    pub skipped_invalid: usize,
}

/// A locally registered skill folder source (persisted in skills.json).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSkillSource {
    /// Unique identifier for this source (assigned at registration time).
    pub id: String,
    /// Absolute path to the folder containing skill subdirectories.
    pub path: String,
    /// Scope: "global" or a project name.
    pub scope: String,
}

/// Lint diagnostics for a single skill directory inside a local source.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSourceLintResult {
    pub skill_name: String,
    pub path: String,
    pub diagnostics: Vec<SkillLintDiagnostic>,
}

/// Character usage of resident instructions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidentUsage {
    pub used_chars: usize,
    pub limit_chars: usize,
    pub warnings: Vec<String>,
}

/// Preview of the resident instructions the model would receive.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResidentPreview {
    pub resident_instructions: String,
    pub skill_names: Vec<String>,
    pub warnings: Vec<String>,
}

/// A subdirectory in the global skills directory that was skipped during discovery.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSkillDirectory {
    pub name: String,
    pub path: String,
    pub reason: String,
}

/// Read-only diagnostics for the global skills folder.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSkillDiagnostics {
    pub global_skills_dir: String,
    pub skill_count: usize,
    pub skipped_subdirectories: Vec<SkippedSkillDirectory>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SkillSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    global_skills_dir: Option<String>,
    #[serde(default)]
    skills: HashMap<String, SkillSetting>,
    /// Locally registered folder sources added via the management API.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    local_sources: Vec<LocalSkillSource>,
}
#[derive(Debug, Serialize, Deserialize)]
struct SkillSetting {
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    options: HashMap<String, Value>,
}
impl Default for SkillSetting {
    fn default() -> Self {
        Self {
            enabled: true,
            options: HashMap::new(),
        }
    }
}

/// Strips Windows extended-length prefix (`\\?\` or `//?/`) from a path.
pub fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let p = path.as_ref();
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else if let Some(stripped) = s.strip_prefix("//?/") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

pub fn resolve_default_global_skills_dir(repository_root: Option<&Path>) -> PathBuf {
    resolve_default_global_skills_dir_from(std::env::current_exe().ok().as_deref(), repository_root)
}

pub fn resolve_default_global_skills_dir_from(
    exe: Option<&Path>,
    repository_root: Option<&Path>,
) -> PathBuf {
    if let Some(env_dir) = std::env::var_os("CHATCMD_GLOBAL_SKILLS_DIR").filter(|s| !s.is_empty()) {
        return normalize_path(PathBuf::from(env_dir));
    }

    if let Some(exe) = exe {
        let exe = normalize_path(exe);
        let is_cargo_target = exe.components().any(|c| c.as_os_str() == "target");
        if is_cargo_target {
            let mut current = exe.as_path();
            while let Some(parent) = current.parent() {
                if parent.file_name().is_some_and(|n| n == "target") {
                    if let Some(project_root) = parent.parent() {
                        return normalize_path(project_root.join("skills").join("global"));
                    }
                }
                current = parent;
            }
            if let Some(repo) = repository_root {
                return normalize_path(repo.join("skills").join("global"));
            }
        } else {
            #[cfg(target_os = "macos")]
            if let Some(app_bundle) = exe.ancestors().find(|p| {
                p.extension()
                    .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("app"))
            }) {
                if let Some(parent) = app_bundle.parent() {
                    return normalize_path(parent.join("skills").join("global"));
                }
            }

            if let Some(parent) = exe.parent() {
                return normalize_path(parent.join("skills").join("global"));
            }
        }
    }

    if let Some(repo) = repository_root {
        normalize_path(repo.join("skills").join("global"))
    } else {
        normalize_path(
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("skills")
                .join("global"),
        )
    }
}

#[derive(Clone)]
pub struct TaskSkillSnapshot {
    pub task_id: Option<String>,
    pub project_folder: Option<PathBuf>,
    pub skills: Vec<DiscoveredSkill>,
    pub summaries: Vec<SkillSummary>,
    pub resident_instructions: String,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct SkillService {
    roots: Vec<(String, PathBuf)>,
    global_roots: Vec<PathBuf>,
    global_skills_dir: PathBuf,
    user_home: Option<PathBuf>,
    project_skills_root: Option<PathBuf>,
    install_root: Option<PathBuf>,
    settings_path: Option<PathBuf>,
    max_characters: usize,
    task_snapshots: Arc<RwLock<HashMap<String, Arc<TaskSkillSnapshot>>>>,
}

#[derive(Clone)]
pub struct DiscoveredSkill {
    pub id: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub directory: PathBuf,
    pub source: String,
    pub source_url: Option<String>,
    pub enabled: bool,
    pub can_delete: bool,
    pub icon_path: Option<String>,
    pub options: Vec<SkillOption>,
    pub precedence: usize,
    pub tier_info: SkillTierInfo,
}

struct InstallCandidateSource {
    candidate: SkillInstallCandidate,
    directory: PathBuf,
}

struct InstallCandidateDiscovery {
    skills: Vec<InstallCandidateSource>,
    skipped_invalid: usize,
}

impl SkillService {
    #[must_use]
    pub fn new(
        user_home: Option<&Path>,
        repository_root: Option<&Path>,
        global_skills_dir: Option<&Path>,
        max_characters: usize,
    ) -> Self {
        let settings_path = user_home.map(|home| home.join(".chatcmd/skills.json"));
        let effective_global = normalize_path(
            global_skills_dir
                .map(PathBuf::from)
                .or_else(|| {
                    settings_path.as_ref().and_then(|p| {
                        if let Ok(content) = fs::read_to_string(p) {
                            let s: SkillSettings = serde_json::from_str(&content).unwrap_or_default();
                            s.global_skills_dir.map(PathBuf::from)
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| resolve_default_global_skills_dir(repository_root)),
        );

        let mut roots = Vec::new();
        let project_skills_root = repository_root
            .map(|repo| normalize_path(repo).join("skills").join("projects"))
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|d| normalize_path(d).join("skills").join("projects"))
            });

        if let Some(repository) = repository_root {
            let repository = normalize_path(repository);
            roots.push(("workspace".into(), repository.join(".agents/skills")));
            roots.push(("workspace".into(), repository.join(".codex/skills")));
        }

        let global_roots = vec![effective_global.clone()];
        roots.push(("global".into(), effective_global.clone()));

        if let Some(home) = user_home {
            roots.push(("user_home".into(), home.join(".agents/skills")));
            roots.push(("user_home".into(), home.join(".codex/skills")));
        }

        let install_root = Some(effective_global.clone());
        Self {
            roots,
            global_roots,
            global_skills_dir: effective_global,
            user_home: user_home.map(Path::to_path_buf),
            project_skills_root,
            install_root,
            settings_path,
            max_characters: max_characters.clamp(1, 1_000_000),
            task_snapshots: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn snapshot_for_task(
        &self,
        task_id: Option<&str>,
        project_folder: Option<&Path>,
    ) -> RuntimeResult<Arc<TaskSkillSnapshot>> {
        if let Some(id) = task_id.filter(|s| !s.trim().is_empty()) {
            let snapshots = self.task_snapshots.read().await;
            if let Some(snapshot) = snapshots.get(id) {
                return Ok(snapshot.clone());
            }
        }

        let roots = self.roots_for_workspace(project_folder);
        let (skills, mut warnings) = self.resolve_skills_for_workspace(&roots)?;

        for skill in &skills {
            if (skill.source == "user_home" || skill.source == "workspace") && skill.tier_info.has_resident {
                warnings.push(format!(
                    "Skill '{}' from legacy source '{}' contains resident.md which is ignored.",
                    skill.name, skill.source
                ));
            }
        }

        let mut active_residents = Vec::new();
        for skill in &skills {
            if skill.enabled {
                if let Some(resident) = &skill.tier_info.resident_content {
                    active_residents.push((skill.name.as_str(), resident.as_str()));
                }
            }
        }

        let (resident_instructions, resident_warnings) =
            merge_resident_instructions(&active_residents);
        warnings.extend(resident_warnings);

        let mut seen = HashSet::new();
        let summaries: Vec<SkillSummary> = skills
            .iter()
            .filter(|skill| skill.enabled && seen.insert(skill.name.to_lowercase()))
            .map(|skill| SkillSummary {
                id: skill.name.clone(),
                name: skill.name.clone(),
                title: skill.title.clone(),
                description: skill.description.clone(),
                source: skill.directory.to_string_lossy().into_owned(),
                has_resident: skill.tier_info.has_resident && skill.tier_info.resident_content.is_some(),
                has_core: skill.tier_info.has_core,
                examples: skill.tier_info.examples.clone(),
            })
            .collect();

        let snapshot = Arc::new(TaskSkillSnapshot {
            task_id: task_id.map(str::to_owned),
            project_folder: project_folder.map(Path::to_path_buf),
            skills,
            summaries,
            resident_instructions,
            warnings,
        });

        if let Some(id) = task_id.filter(|s| !s.trim().is_empty()) {
            let mut snapshots = self.task_snapshots.write().await;
            snapshots.insert(id.to_owned(), snapshot.clone());
        }

        Ok(snapshot)
    }

    pub async fn list(&self) -> RuntimeResult<Vec<SkillSummary>> {
        self.list_from_roots(&self.roots)
    }

    pub async fn list_for_workspace(
        &self,
        repository_root: Option<&Path>,
    ) -> RuntimeResult<Vec<SkillSummary>> {
        self.list_for_task(None, repository_root).await
    }

    pub async fn list_for_task(
        &self,
        task_id: Option<&str>,
        project_folder: Option<&Path>,
    ) -> RuntimeResult<Vec<SkillSummary>> {
        let snapshot = self.snapshot_for_task(task_id, project_folder).await?;
        Ok(snapshot.summaries.clone())
    }

    pub async fn read(&self, skill_id: &str) -> RuntimeResult<SkillReadResult> {
        self.read_for_task(None, skill_id, None, None, None).await
    }

    pub async fn read_for_workspace(
        &self,
        skill_id: &str,
        repository_root: Option<&Path>,
    ) -> RuntimeResult<SkillReadResult> {
        self.read_for_task(None, skill_id, None, None, repository_root)
            .await
    }

    pub async fn read_for_task(
        &self,
        task_id: Option<&str>,
        skill_id: &str,
        tier: Option<&str>,
        tool: Option<&str>,
        project_folder: Option<&Path>,
    ) -> RuntimeResult<SkillReadResult> {
        let trimmed_skill_id = skill_id.trim();
        if trimmed_skill_id.is_empty() {
            return Err(RuntimeError::new(
                "invalid_skill_id",
                "skillId cannot be empty",
            ));
        }
        if trimmed_skill_id.contains("..")
            || trimmed_skill_id.contains('/')
            || trimmed_skill_id.contains('\\')
            || trimmed_skill_id.contains(':')
        {
            return Err(RuntimeError::new(
                "invalid_skill_id",
                "skillId cannot contain '..', slashes, or colons",
            ));
        }

        let tier_normalized = tier.map(str::trim).filter(|v| !v.is_empty()).unwrap_or("core");
        if tier_normalized != "core" && tier_normalized != "examples" {
            return Err(RuntimeError::new(
                "invalid_tier",
                format!("Invalid skill tier '{tier_normalized}'. Only 'core' and 'examples' are allowed."),
            ));
        }

        if let Some(tool_raw) = tool {
            let tool_name = tool_raw.trim();
            if tool_name.is_empty() {
                return Err(RuntimeError::new(
                    "invalid_tool_name",
                    "tool name cannot be empty string",
                ));
            }
            if tool_name.contains("..")
                || tool_name.contains('/')
                || tool_name.contains('\\')
                || tool_name.contains(':')
            {
                return Err(RuntimeError::new(
                    "invalid_tool_name",
                    "tool name cannot contain '..', slashes, or colons",
                ));
            }
            if !tool_name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(RuntimeError::new(
                    "invalid_tool_name",
                    "tool name only allows lowercase letters, numbers, and underscores",
                ));
            }
        }

        let snapshot = self.snapshot_for_task(task_id, project_folder).await?;
        let selected = snapshot
            .skills
            .iter()
            .find(|skill| skill.id == trimmed_skill_id || skill.name.eq_ignore_ascii_case(trimmed_skill_id))
            .ok_or_else(|| {
                RuntimeError::new("skill_not_found", "skill is unavailable or shadowed")
            })?;

        match tier_normalized {
            "core" => {
                let file = selected
                    .tier_info
                    .core_path
                    .clone()
                    .unwrap_or_else(|| selected.directory.join("SKILL.md"));
                let content = tokio::fs::read_to_string(&file).await.map_err(io_error)?;
                let truncated = content.chars().count() > self.max_characters;
                Ok(SkillReadResult {
                    id: selected.id.clone(),
                    name: selected.name.clone(),
                    source: selected.directory.to_string_lossy().into_owned(),
                    instructions: content.chars().take(self.max_characters).collect(),
                    truncated,
                    examples: None,
                })
            }
            "examples" => {
                if let Some(tool_raw) = tool {
                    let tool_name = tool_raw.trim();
                    let examples_dir = selected.directory.join("examples");
                    if !examples_dir.is_dir() {
                        return Err(RuntimeError::new(
                            "example_not_found",
                            format!(
                                "Example for tool '{tool_name}' not found in skill '{trimmed_skill_id}'"
                            ),
                        ));
                    }
                    let canonical_examples_dir = examples_dir.canonicalize().map_err(io_error)?;

                    let example_file = examples_dir.join(format!("{tool_name}.md"));
                    let fallback_file = examples_dir.join(tool_name);
                    let target_file = if example_file.is_file() {
                        example_file
                    } else if fallback_file.is_file() {
                        fallback_file
                    } else {
                        return Err(RuntimeError::new(
                            "example_not_found",
                            format!(
                                "Example for tool '{tool_name}' not found in skill '{trimmed_skill_id}'"
                            ),
                        ));
                    };

                    let canonical_target = target_file.canonicalize().map_err(io_error)?;
                    if !canonical_target.starts_with(&canonical_examples_dir) {
                        return Err(RuntimeError::new(
                            "path_traversal_denied",
                            "Resolved example path escapes the skill's examples directory",
                        ));
                    }

                    let content = tokio::fs::read_to_string(&canonical_target)
                        .await
                        .map_err(io_error)?;
                    let truncated = content.chars().count() > self.max_characters;
                    Ok(SkillReadResult {
                        id: selected.id.clone(),
                        name: selected.name.clone(),
                        source: selected.directory.to_string_lossy().into_owned(),
                        instructions: content.chars().take(self.max_characters).collect(),
                        truncated,
                        examples: None,
                    })
                } else {
                    let mut instructions = String::new();
                    if selected.tier_info.examples.is_empty() {
                        instructions =
                            format!("No tool examples available for skill '{trimmed_skill_id}'.");
                    } else {
                        instructions.push_str(&format!(
                            "Available tool examples for skill '{trimmed_skill_id}':\n"
                        ));
                        for ex in &selected.tier_info.examples {
                            instructions.push_str(&format!("- {ex}\n"));
                        }
                        instructions.push_str(
                            "\nCall skill_read with tier: \"examples\" and tool: \"<tool_name>\" to read an example.",
                        );
                    }
                    Ok(SkillReadResult {
                        id: selected.id.clone(),
                        name: selected.name.clone(),
                        source: selected.directory.to_string_lossy().into_owned(),
                        instructions,
                        truncated: false,
                        examples: Some(selected.tier_info.examples.clone()),
                    })
                }
            }
            _ => unreachable!(),
        }
    }

    pub async fn active_resident_instructions_for_task(
        &self,
        task_id: Option<&str>,
        project_folder: Option<&Path>,
    ) -> RuntimeResult<(String, Vec<String>)> {
        let snapshot = self.snapshot_for_task(task_id, project_folder).await?;
        Ok((
            snapshot.resident_instructions.clone(),
            snapshot.warnings.clone(),
        ))
    }

    pub fn lint_skill(
        &self,
        directory: &Path,
        source_type: SkillSourceType,
    ) -> Vec<SkillLintDiagnostic> {
        let (_info, diagnostics) = inspect_skill_tier(directory, source_type);
        diagnostics
    }

    pub fn global_skills_dir(&self) -> &Path {
        &self.global_skills_dir
    }

    pub async fn list_global(&self) -> RuntimeResult<Vec<ManagedSkill>> {
        let mut values: Vec<_> = self
            .discover_all()?
            .into_iter()
            .filter(|skill| skill.source == "global" || skill.source == "user_home")
            .collect();
        values.sort_by_key(|skill| (skill.precedence, skill.name.to_lowercase()));
        let mut seen = HashSet::new();
        Ok(values
            .into_iter()
            .filter(|skill| seen.insert(skill.name.to_lowercase()))
            .map(to_managed)
            .collect())
    }

    pub async fn set_enabled(
        &self,
        id: &str,
        enabled: bool,
    ) -> RuntimeResult<Option<ManagedSkill>> {
        let Some(skill) = self.global_by_id(id)? else {
            return Ok(None);
        };
        let mut settings = self.load_settings()?;
        settings
            .skills
            .entry(skill_key(&skill))
            .or_default()
            .enabled = enabled;
        self.save_settings(&settings)?;
        let mut skill = skill;
        skill.enabled = enabled;
        Ok(Some(to_managed(skill)))
    }

    pub async fn set_options(
        &self,
        id: &str,
        values: HashMap<String, Value>,
    ) -> RuntimeResult<Option<ManagedSkill>> {
        let Some(mut skill) = self.global_by_id(id)? else {
            return Ok(None);
        };
        let definitions = option_definitions(&skill.name);
        if definitions.is_empty() && !values.is_empty() {
            return Err(RuntimeError::new(
                "invalid_skill_options",
                "This skill does not expose configurable options.",
            ));
        }
        for (key, value) in &values {
            let Some(definition) = definitions.iter().find(|item| item.key == key) else {
                return Err(RuntimeError::new(
                    "invalid_skill_options",
                    format!("Unknown skill option '{key}'."),
                ));
            };
            if definition.option_type == "select" {
                let Some(text) = value.as_str() else {
                    return Err(RuntimeError::new(
                        "invalid_skill_options",
                        format!("Invalid value for option '{key}'."),
                    ));
                };
                if !definition.choices.contains(&text) {
                    return Err(RuntimeError::new(
                        "invalid_skill_options",
                        format!("Invalid value for option '{key}'."),
                    ));
                }
            }
        }
        let mut settings = self.load_settings()?;
        settings
            .skills
            .entry(skill_key(&skill))
            .or_default()
            .options = values.clone();
        self.save_settings(&settings)?;
        skill.options = create_options(&skill.name, Some(&values));
        Ok(Some(to_managed(skill)))
    }

    pub async fn delete(&self, id: &str) -> RuntimeResult<bool> {
        let Some(skill) = self.global_by_id(id)? else {
            return Ok(false);
        };
        if !skill.can_delete {
            return Err(RuntimeError::new(
                "skill_delete_denied",
                "This skill cannot be deleted by ChatCMD.",
            ));
        }
        fs::remove_dir_all(&skill.directory).map_err(io_error)?;
        let mut settings = self.load_settings()?;
        settings.skills.remove(&skill_key(&skill));
        self.save_settings(&settings)?;
        Ok(true)
    }

    pub async fn icon(&self, id: &str) -> RuntimeResult<Option<(PathBuf, &'static str)>> {
        let Some(skill) = self.global_by_id(id)? else {
            return Ok(None);
        };
        let Some(path) = skill.icon_path.map(PathBuf::from) else {
            return Ok(None);
        };
        let content_type = match path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            _ => "application/octet-stream",
        };
        Ok(Some((path, content_type)))
    }

    pub async fn preview_install(
        &self,
        repository_url: &str,
    ) -> RuntimeResult<SkillInstallPreview> {
        let source = parse_github_url(repository_url)?;
        let checkout = clone_repository(&source).await?;
        let clone_root = checkout.path().join("repo");
        let candidate_root = source
            .subdirectory
            .as_deref()
            .map_or_else(|| clone_root.clone(), |path| clone_root.join(path));
        if !candidate_root.is_dir() {
            return Err(RuntimeError::new(
                "invalid_skill_repository",
                "The GitHub subdirectory does not exist in the selected repository revision.",
            ));
        }
        let installed_names = self
            .discover_all()?
            .into_iter()
            .filter(|skill| skill.source == "global")
            .map(|skill| skill.name.to_lowercase())
            .collect();
        let discovery =
            discover_install_candidates(&candidate_root, &clone_root, &installed_names)?;
        if discovery.skills.is_empty() {
            return Err(RuntimeError::new(
                "invalid_skill_repository",
                "No valid skills were found. Each skill directory must contain a SKILL.md with a lowercase name and description.",
            ));
        }
        Ok(SkillInstallPreview {
            repository_url: source.repository_url,
            skills: discovery
                .skills
                .into_iter()
                .map(|skill| skill.candidate)
                .collect(),
            skipped_invalid: discovery.skipped_invalid,
        })
    }

    pub async fn install(
        &self,
        repository_url: &str,
        skill_paths: &[String],
    ) -> RuntimeResult<Vec<ManagedSkill>> {
        let source = parse_github_url(repository_url)?;
        let install_root = self.install_root.as_ref().ok_or_else(|| {
            RuntimeError::new("skill_install_unavailable", "User home is unavailable.")
        })?;
        let checkout = clone_repository(&source).await?;
        let clone_root = checkout.path().join("repo");
        let candidate_root = source
            .subdirectory
            .as_deref()
            .map_or_else(|| clone_root.clone(), |path| clone_root.join(path));
        if !candidate_root.is_dir() {
            return Err(RuntimeError::new(
                "invalid_skill_repository",
                "The GitHub subdirectory does not exist in the selected repository revision.",
            ));
        }
        let installed_names = self
            .discover_all()?
            .into_iter()
            .filter(|skill| skill.source == "global")
            .map(|skill| skill.name.to_lowercase())
            .collect();
        let discovery =
            discover_install_candidates(&candidate_root, &clone_root, &installed_names)?;
        if discovery.skills.is_empty() {
            return Err(RuntimeError::new(
                "invalid_skill_repository",
                "No valid skills were found. Each skill directory must contain a SKILL.md with a lowercase name and description.",
            ));
        }
        let requested_paths: Vec<&str> = if skill_paths.is_empty() {
            if discovery.skills.len() != 1 {
                return Err(RuntimeError::new(
                    "skill_selection_required",
                    "Repository contains multiple skills. Preview it and choose which skills to install.",
                ));
            }
            vec![discovery.skills[0].candidate.path.as_str()]
        } else {
            skill_paths.iter().map(String::as_str).collect()
        };
        let unique_paths: HashSet<_> = requested_paths.iter().copied().collect();
        if unique_paths.len() != requested_paths.len()
            || requested_paths.len() > MAX_DISCOVERED_SKILLS
        {
            return Err(RuntimeError::new(
                "invalid_skill_selection",
                "Select between 1 and 200 unique skill paths.",
            ));
        }
        let selected: Vec<_> = discovery
            .skills
            .iter()
            .filter(|skill| unique_paths.contains(skill.candidate.path.as_str()))
            .collect();
        if selected.len() != requested_paths.len() {
            return Err(RuntimeError::new(
                "invalid_skill_selection",
                "One or more selected skill paths are not available in this repository.",
            ));
        }
        let mut selected_names = HashSet::new();
        for skill in &selected {
            if skill.candidate.installed || install_root.join(&skill.candidate.name).exists() {
                return Err(RuntimeError::new(
                    "skill_conflict",
                    format!("Skill '{}' is already installed.", skill.candidate.name),
                ));
            }
            if !selected_names.insert(skill.candidate.name.to_lowercase()) {
                return Err(RuntimeError::new(
                    "invalid_skill_selection",
                    format!(
                        "More than one selected directory declares the skill name '{}'.",
                        skill.candidate.name
                    ),
                ));
            }
        }

        let installed_destinations =
            install_candidate_directories(install_root, &selected, &source.repository_url)?;

        let mut installed = Vec::with_capacity(selected.len());
        for skill in selected {
            let discovered = match self.global_by_name(&skill.candidate.name) {
                Ok(Some(discovered)) => discovered,
                Ok(None) => {
                    rollback_install(&installed_destinations);
                    return Err(RuntimeError::new(
                        "skill_install_failed",
                        "Installed skills could not be discovered.",
                    ));
                }
                Err(error) => {
                    rollback_install(&installed_destinations);
                    return Err(error);
                }
            };
            installed.push(to_managed(discovered));
        }
        Ok(installed)
    }

    fn global_by_id(&self, id: &str) -> RuntimeResult<Option<DiscoveredSkill>> {
        Ok(self
            .discover_all()?
            .into_iter()
            .find(|skill| (skill.source == "global" || skill.source == "user_home") && skill.id == id))
    }
    fn global_by_name(&self, name: &str) -> RuntimeResult<Option<DiscoveredSkill>> {
        Ok(self
            .discover_all()?
            .into_iter()
            .find(|skill| (skill.source == "global" || skill.source == "user_home") && skill.name.eq_ignore_ascii_case(name)))
    }

    fn roots_for_workspace(&self, repository_root: Option<&Path>) -> Vec<(String, PathBuf)> {
        let mut roots = Vec::new();
        if let Some(repository) = repository_root {
            let project_name = repository
                .file_name()
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !project_name.is_empty() {
                if let Some(project_root) = &self.project_skills_root {
                    let dir = project_root.join(&project_name);
                    if dir.is_dir() {
                        roots.push(("project".into(), dir));
                    }
                }
                if let Some(home_parent) = &self.settings_path.as_ref().and_then(|p| p.parent()) {
                    let dir = home_parent.join("skills").join("projects").join(&project_name);
                    if dir.is_dir() && !roots.iter().any(|(_, p)| p == &dir) {
                        roots.push(("project".into(), dir));
                    }
                }
            }
            roots.push(("workspace".into(), repository.join(".agents/skills")));
            roots.push(("workspace".into(), repository.join(".codex/skills")));
        }
        roots.extend(
            self.global_roots
                .iter()
                .cloned()
                .map(|root| ("global".into(), root)),
        );
        if let Some(home) = &self.user_home {
            roots.push(("user_home".into(), home.join(".agents/skills")));
            roots.push(("user_home".into(), home.join(".codex/skills")));
        }
        roots
    }

    fn resolve_skills_for_workspace(
        &self,
        roots: &[(String, PathBuf)],
    ) -> RuntimeResult<(Vec<DiscoveredSkill>, Vec<String>)> {
        let all = self.discover_from_roots(roots)?;
        let mut warnings = Vec::new();

        let mut project_skills = HashMap::new();
        let mut other_skills = Vec::new();

        for skill in all {
            if skill.source == "project" {
                project_skills.insert(skill.name.to_lowercase(), skill);
            } else {
                other_skills.push(skill);
            }
        }

        other_skills.sort_by_key(|skill| (skill.precedence, skill.name.to_lowercase()));

        let mut final_skills = Vec::new();
        let mut seen = HashSet::new();

        for global_skill in other_skills {
            let key = global_skill.name.to_lowercase();
            if let Some(proj_skill) = project_skills.remove(&key) {
                let target_match = proj_skill
                    .tier_info
                    .overrides
                    .as_deref()
                    .is_some_and(|target| target.eq_ignore_ascii_case(&global_skill.name));

                if target_match {
                    if seen.insert(key) {
                        final_skills.push(proj_skill);
                    }
                } else {
                    warnings.push(format!(
                        "Project skill '{}' does not override global skill because frontmatter lacks 'overrides: {}'.",
                        proj_skill.name, global_skill.name
                    ));
                    if seen.insert(key) {
                        final_skills.push(global_skill);
                    }
                }
            } else if seen.insert(key) {
                final_skills.push(global_skill);
            }
        }

        for (key, proj_skill) in project_skills {
            if seen.insert(key) {
                final_skills.push(proj_skill);
            }
        }

        Ok((final_skills, warnings))
    }

    fn list_from_roots(&self, roots: &[(String, PathBuf)]) -> RuntimeResult<Vec<SkillSummary>> {
        let (all, _warnings) = self.resolve_skills_for_workspace(roots)?;
        let mut seen = HashSet::new();
        Ok(all
            .into_iter()
            .filter(|skill| skill.enabled && seen.insert(skill.name.to_lowercase()))
            .map(|skill| SkillSummary {
                id: skill.name.clone(),
                name: skill.name,
                title: skill.title,
                description: skill.description,
                source: skill.directory.to_string_lossy().into_owned(),
                has_resident: skill.tier_info.has_resident && skill.tier_info.resident_content.is_some(),
                has_core: skill.tier_info.has_core,
                examples: skill.tier_info.examples,
            })
            .collect())
    }

    fn discover_all(&self) -> RuntimeResult<Vec<DiscoveredSkill>> {
        self.discover_from_roots(&self.roots)
    }

    fn discover_from_roots(
        &self,
        roots: &[(String, PathBuf)],
    ) -> RuntimeResult<Vec<DiscoveredSkill>> {
        let settings = self.load_settings()?;
        let mut values = Vec::new();
        for (index, (source, root)) in roots.iter().enumerate() {
            let Ok(entries) = fs::read_dir(root) else {
                continue;
            };
            let mut directories: Vec<_> = entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .map(|entry| entry.path())
                .collect();
            directories.sort_by_key(|path| {
                path.file_name()
                    .map(|v| v.to_string_lossy().to_lowercase())
                    .unwrap_or_default()
            });
            for directory in directories {
                if directory
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with('.'))
                {
                    continue;
                }
                if !directory.join("SKILL.md").is_file() && !directory.join("core.md").is_file() {
                    continue;
                }
                if let Ok(skill) = self.parse_skill(directory, source, index, &settings) {
                    values.push(skill);
                }
            }
        }
        Ok(values)
    }

    fn parse_skill(
        &self,
        directory: PathBuf,
        source: &str,
        precedence: usize,
        settings: &SkillSettings,
    ) -> RuntimeResult<DiscoveredSkill> {
        let source_type = if source == "project" {
            SkillSourceType::Project
        } else if source == "global" {
            SkillSourceType::Global
        } else {
            SkillSourceType::LegacyAgentOrCodex
        };

        let (tier_info, _diagnostics) = inspect_skill_tier(&directory, source_type);
        let file = tier_info
            .core_path
            .clone()
            .unwrap_or_else(|| directory.join("SKILL.md"));

        if fs::metadata(&file).map_err(io_error)?.len() > MAX_SKILL_BYTES {
            return Err(RuntimeError::new(
                "skill_too_large",
                "Skill instruction file exceeds 2 MB.",
            ));
        }
        let metadata = parse_frontmatter(&fs::read_to_string(&file).map_err(io_error)?);
        let fallback = directory
            .file_name()
            .map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_else(|| "skill".into());
        let name = metadata
            .get("name")
            .cloned()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| fallback.clone());
        let title = openai_value(&directory, "display_name").unwrap_or_else(|| name.clone());
        let key = format!("{source}:{}", directory.to_string_lossy());
        let stored = settings.skills.get(&key);
        let icon_path = openai_value(&directory, "icon_small")
            .and_then(|value| resolve_icon(&directory, &value));
        let can_delete = source == "global"
            && self
                .global_roots
                .iter()
                .any(|root| directory.starts_with(root))
            && !directory
                .components()
                .any(|part| part.as_os_str() == ".system");
        Ok(DiscoveredSkill {
            id: name.clone(),
            name: name.clone(),
            title,
            description: metadata.get("description").cloned().unwrap_or_default(),
            directory: directory.clone(),
            source: source.into(),
            source_url: source_url(&directory),
            enabled: stored.map(|v| v.enabled).unwrap_or(true),
            can_delete,
            icon_path,
            options: create_options(&name, stored.map(|v| &v.options)),
            precedence,
            tier_info,
        })
    }

    fn load_settings(&self) -> RuntimeResult<SkillSettings> {
        let Some(path) = &self.settings_path else {
            return Ok(SkillSettings::default());
        };
        if !path.exists() {
            return Ok(SkillSettings::default());
        }
        serde_json::from_slice(&fs::read(path).map_err(io_error)?)
            .map_err(|error| RuntimeError::new("skill_settings_invalid", error.to_string()))
    }
    fn save_settings(&self, settings: &SkillSettings) -> RuntimeResult<()> {
        let Some(path) = &self.settings_path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        fs::write(
            path,
            serde_json::to_vec_pretty(settings)
                .map_err(|error| RuntimeError::new("skill_settings_invalid", error.to_string()))?,
        )
        .map_err(io_error)
    }

    // ── Local source management ──────────────────────────────────────────────

    /// List all locally-registered folder sources.
    pub fn list_local_sources(&self) -> RuntimeResult<Vec<LocalSkillSource>> {
        Ok(self.load_settings()?.local_sources)
    }

    /// Register a new local folder source.
    ///
    /// Validation:
    /// - The folder must exist and be readable.
    /// - It must contain at least one valid skill (directory with SKILL.md or core.md).
    /// - The path is stored exactly as supplied (normalised to absolute); no files are copied.
    ///
    /// Returns the newly created `LocalSkillSource` on success.
    pub fn add_local_source(
        &self,
        raw_path: &str,
        scope: &str,
    ) -> RuntimeResult<LocalSkillSource> {
        // Reject obviously dangerous path fragments.
        if raw_path.contains("..") {
            return Err(RuntimeError::new(
                "invalid_local_source",
                "Path must not contain '..'",
            ));
        }
        let path = PathBuf::from(raw_path);
        if !path.is_absolute() {
            return Err(RuntimeError::new(
                "invalid_local_source",
                "Path must be absolute",
            ));
        }
        // Must be an existing directory.
        if !path.is_dir() {
            return Err(RuntimeError::new(
                "invalid_local_source",
                "Path does not exist or is not a directory",
            ));
        }
        // Must contain at least one valid skill subdirectory.
        let has_any_skill = fs::read_dir(&path)
            .map_err(io_error)?
            .filter_map(Result::ok)
            .any(|entry| {
                let dir = entry.path();
                dir.is_dir()
                    && (dir.join("SKILL.md").is_file() || dir.join("core.md").is_file())
            });
        if !has_any_skill {
            return Err(RuntimeError::new(
                "invalid_local_source",
                "Directory contains no valid skill subdirectories (each must have SKILL.md or core.md)",
            ));
        }

        let mut settings = self.load_settings()?;
        // Reject duplicates.
        let canonical = normalize_path(path.canonicalize().map_err(io_error)?);
        for existing in &settings.local_sources {
            if let Ok(existing_canonical) = PathBuf::from(&existing.path).canonicalize() {
                if normalize_path(&existing_canonical) == canonical {
                    return Err(RuntimeError::new(
                        "local_source_conflict",
                        "This path is already registered as a local source",
                    ));
                }
            }
        }

        let id = format!(
            "local-{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis())
        );
        let source = LocalSkillSource {
            id: id.clone(),
            path: canonical.to_string_lossy().into_owned(),
            scope: scope.to_owned(),
        };
        settings.local_sources.push(source.clone());
        self.save_settings(&settings)?;
        Ok(source)
    }

    /// Remove a locally-registered source by its ID.
    /// Only deregisters; never deletes files on disk.
    pub fn remove_local_source(&self, source_id: &str) -> RuntimeResult<bool> {
        let mut settings = self.load_settings()?;
        let before = settings.local_sources.len();
        settings
            .local_sources
            .retain(|s| s.id != source_id);
        if settings.local_sources.len() == before {
            return Ok(false);
        }
        self.save_settings(&settings)?;
        Ok(true)
    }

    /// Reload a local source — invalidates task snapshots that include the source.
    ///
    /// At the moment, invalidating all snapshots is the conservative approach.
    /// A future version could only evict snapshots whose roots overlap the source.
    pub async fn reload_local_source(&self, source_id: &str) -> RuntimeResult<bool> {
        let settings = self.load_settings()?;
        let exists = settings.local_sources.iter().any(|s| s.id == source_id);
        if !exists {
            return Ok(false);
        }
        // Evict all cached task snapshots so they are rebuilt on next access.
        self.task_snapshots.write().await.clear();
        Ok(true)
    }

    /// Evict every task snapshot and rediscover skills from disk.
    pub async fn reload_all(&self) -> RuntimeResult<usize> {
        self.task_snapshots.write().await.clear();
        Ok(self.list_global().await?.len())
    }

    /// Run lint on every skill directory inside a registered local source.
    /// Returns diagnostics grouped by skill name.
    pub fn lint_local_source(
        &self,
        source_id: &str,
    ) -> RuntimeResult<Vec<SkillSourceLintResult>> {
        let settings = self.load_settings()?;
        let Some(source) = settings.local_sources.iter().find(|s| s.id == source_id) else {
            return Err(RuntimeError::new(
                "local_source_not_found",
                "No local source with that id",
            ));
        };
        let root = PathBuf::from(&source.path);
        let source_type = SkillSourceType::Global; // local sources behave like global

        let entries = match fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            if dir
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
            {
                continue;
            }
            if !dir.join("SKILL.md").is_file() && !dir.join("core.md").is_file() {
                continue;
            }
            let diagnostics = self.lint_skill(&dir, source_type);
            let skill_name = dir
                .file_name()
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_default();
            results.push(SkillSourceLintResult {
                skill_name,
                path: normalize_path(&dir).to_string_lossy().into_owned(),
                diagnostics,
            });
        }
        results.sort_by(|a, b| a.skill_name.cmp(&b.skill_name));
        Ok(results)
    }

    /// Return the current resident-instruction character usage and limit.
    pub async fn resident_usage(
        &self,
        task_id: Option<&str>,
        project_folder: Option<&Path>,
    ) -> RuntimeResult<ResidentUsage> {
        let snapshot = self.snapshot_for_task(task_id, project_folder).await?;
        let used = snapshot.resident_instructions.chars().count();
        Ok(ResidentUsage {
            used_chars: used,
            limit_chars: MAX_RESIDENT_CHARS_TOTAL,
            warnings: snapshot.warnings.clone(),
        })
    }

    /// Preview the resident instructions a model would receive for a given project folder.
    /// Calls the same `snapshot_for_task` path used in real conversations.
    pub async fn resident_preview_for_project(
        &self,
        project_folder: Option<&Path>,
    ) -> RuntimeResult<ResidentPreview> {
        let snapshot = self.snapshot_for_task(None, project_folder).await?;
        Ok(ResidentPreview {
            resident_instructions: snapshot.resident_instructions.clone(),
            skill_names: snapshot
                .skills
                .iter()
                .filter(|s| s.enabled && s.tier_info.resident_content.is_some())
                .map(|s| s.name.clone())
                .collect(),
            warnings: snapshot.warnings.clone(),
        })
    }

    /// Return read-only diagnostics for the configured global skills directory.
    /// Only returns directory names, paths, and skip reasons; never file contents.
    pub fn global_diagnostics(&self) -> RuntimeResult<GlobalSkillDiagnostics> {
        let global_dir = normalize_path(self.global_skills_dir.clone());
        let global_skills_dir = global_dir.to_string_lossy().into_owned();
        if !global_dir.is_dir() {
            return Ok(GlobalSkillDiagnostics {
                global_skills_dir,
                skill_count: 0,
                skipped_subdirectories: Vec::new(),
            });
        }

        let settings = self.load_settings()?;
        let entries = match fs::read_dir(&global_dir) {
            Ok(e) => e,
            Err(_) => {
                return Ok(GlobalSkillDiagnostics {
                    global_skills_dir,
                    skill_count: 0,
                    skipped_subdirectories: Vec::new(),
                });
            }
        };

        let mut subdirectories: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();
        subdirectories.sort_by_key(|p| {
            p.file_name()
                .map(|v| v.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        });

        let mut skill_count = 0;
        let mut skipped_subdirectories = Vec::new();
        let mut seen_names = HashSet::new();

        for dir in subdirectories {
            let dir_name = dir
                .file_name()
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_default();
            let dir_path = normalize_path(&dir).to_string_lossy().into_owned();

            if dir_name.starts_with('.') {
                skipped_subdirectories.push(SkippedSkillDirectory {
                    name: dir_name,
                    path: dir_path,
                    reason: "Hidden or system directory (starts with '.')".into(),
                });
                continue;
            }

            let skill_file = if dir.join("core.md").is_file() {
                dir.join("core.md")
            } else if dir.join("SKILL.md").is_file() {
                dir.join("SKILL.md")
            } else {
                skipped_subdirectories.push(SkippedSkillDirectory {
                    name: dir_name,
                    path: dir_path,
                    reason: "Missing SKILL.md or core.md".into(),
                });
                continue;
            };

            let meta = match fs::metadata(&skill_file) {
                Ok(m) => m,
                Err(err) => {
                    skipped_subdirectories.push(SkippedSkillDirectory {
                        name: dir_name,
                        path: dir_path,
                        reason: format!("Cannot read file metadata: {err}"),
                    });
                    continue;
                }
            };

            if meta.len() > MAX_SKILL_BYTES {
                skipped_subdirectories.push(SkippedSkillDirectory {
                    name: dir_name,
                    path: dir_path,
                    reason: format!(
                        "Skill instruction file exceeds limit of {} MB",
                        MAX_SKILL_BYTES / 1_000_000
                    ),
                });
                continue;
            }

            let content = match fs::read_to_string(&skill_file) {
                Ok(c) => c,
                Err(err) => {
                    skipped_subdirectories.push(SkippedSkillDirectory {
                        name: dir_name,
                        path: dir_path,
                        reason: format!("Cannot read file: {err}"),
                    });
                    continue;
                }
            };

            let frontmatter = parse_frontmatter(&content);
            let name = frontmatter
                .get("name")
                .cloned()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| dir_name.clone());

            if !valid_skill_name(&name) {
                skipped_subdirectories.push(SkippedSkillDirectory {
                    name: dir_name,
                    path: dir_path,
                    reason: format!(
                        "Invalid skill name '{name}' (must be 1-64 chars, lowercase alphanumeric, underscore, hyphen)"
                    ),
                });
                continue;
            }

            // Check if disabled in settings
            let key = format!("global:{}", dir.to_string_lossy());
            if let Some(stored) = settings.skills.get(&key) {
                if !stored.enabled {
                    skipped_subdirectories.push(SkippedSkillDirectory {
                        name: dir_name,
                        path: dir_path,
                        reason: format!("Disabled in skill settings (skill '{name}')"),
                    });
                    continue;
                }
            }

            // Check if duplicate skill name within global directory
            let name_lower = name.to_lowercase();
            if !seen_names.insert(name_lower) {
                skipped_subdirectories.push(SkippedSkillDirectory {
                    name: dir_name,
                    path: dir_path,
                    reason: format!(
                        "Duplicate skill name '{name}' (shadowed by another directory)"
                    ),
                });
                continue;
            }

            skill_count += 1;
        }

        Ok(GlobalSkillDiagnostics {
            global_skills_dir,
            skill_count,
            skipped_subdirectories,
        })
    }
}

async fn clone_repository(source: &GitHubSource) -> RuntimeResult<tempfile::TempDir> {
    let checkout = tempfile::Builder::new()
        .prefix("chatcmd-skill-")
        .tempdir()
        .map_err(io_error)?;
    let clone_root = checkout.path().join("repo");
    let mut args = vec![
        "clone".to_owned(),
        "--depth".into(),
        "1".into(),
        "--single-branch".into(),
        "--filter=blob:none".into(),
    ];
    if let Some(reference) = &source.reference {
        args.extend(["--branch".into(), reference.clone()]);
    }
    args.extend([
        "--".into(),
        source.clone_url.clone(),
        clone_root.to_string_lossy().into_owned(),
    ]);
    let output = Command::new("git")
        .args(&args)
        .current_dir(checkout.path())
        .output()
        .await
        .map_err(io_error)?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr)
            .trim()
            .chars()
            .take(2_000)
            .collect::<String>();
        return Err(RuntimeError::new(
            "skill_clone_failed",
            if detail.is_empty() {
                "Git could not clone the selected repository.".into()
            } else {
                detail
            },
        ));
    }
    Ok(checkout)
}

fn discover_install_candidates(
    candidate_root: &Path,
    clone_root: &Path,
    installed_names: &HashSet<String>,
) -> RuntimeResult<InstallCandidateDiscovery> {
    let mut skill_files = Vec::new();
    collect_skill_files(candidate_root, &mut skill_files)?;
    skill_files.sort();
    let mut skills = Vec::with_capacity(skill_files.len());
    let mut skipped_invalid = 0usize;
    for skill_file in skill_files {
        if fs::metadata(&skill_file).map_err(io_error)?.len() > MAX_SKILL_BYTES {
            skipped_invalid += 1;
            continue;
        }
        let Some(directory) = skill_file.parent() else {
            skipped_invalid += 1;
            continue;
        };
        let metadata = parse_frontmatter(&fs::read_to_string(&skill_file).map_err(io_error)?);
        let name = metadata.get("name").cloned().unwrap_or_default();
        let description = metadata.get("description").cloned().unwrap_or_default();
        if !valid_skill_name(&name) || description.trim().is_empty() {
            skipped_invalid += 1;
            continue;
        }
        if let Err(error) = validate_install_tree(directory) {
            if matches!(
                error.code.as_str(),
                "invalid_skill_repository" | "skill_too_large"
            ) {
                skipped_invalid += 1;
                continue;
            }
            return Err(error);
        }
        let relative = directory.strip_prefix(clone_root).map_err(|_| {
            RuntimeError::new(
                "invalid_skill_repository",
                "A discovered skill is outside the cloned repository.",
            )
        })?;
        let path = if relative.as_os_str().is_empty() {
            ".".into()
        } else {
            relative.to_string_lossy().replace('\\', "/")
        };
        let title = openai_value(directory, "display_name").unwrap_or_else(|| name.clone());
        skills.push(InstallCandidateSource {
            candidate: SkillInstallCandidate {
                installed: installed_names.contains(&name.to_lowercase()),
                name,
                title,
                description: description.trim().to_owned(),
                path,
            },
            directory: directory.to_path_buf(),
        });
    }
    Ok(InstallCandidateDiscovery {
        skills,
        skipped_invalid,
    })
}

fn install_candidate_directories(
    install_root: &Path,
    selected: &[&InstallCandidateSource],
    repository_url: &str,
) -> RuntimeResult<Vec<PathBuf>> {
    fs::create_dir_all(install_root).map_err(io_error)?;
    let staging = tempfile::Builder::new()
        .prefix(".chatcmd-skill-install-")
        .tempdir_in(install_root)
        .map_err(io_error)?;
    for skill in selected {
        let staged_destination = staging.path().join(&skill.candidate.name);
        copy_tree(&skill.directory, &staged_destination)?;
        let source_metadata = serde_json::to_vec_pretty(&serde_json::json!({
            "repositoryUrl": repository_url,
            "skillPath": skill.candidate.path.as_str(),
        }))
        .map_err(|error| RuntimeError::new("skill_install_failed", error.to_string()))?;
        fs::write(
            staged_destination.join(".cmdgpt-source.json"),
            source_metadata,
        )
        .map_err(io_error)?;
    }

    let mut installed_destinations = Vec::with_capacity(selected.len());
    for skill in selected {
        let destination = install_root.join(&skill.candidate.name);
        if let Err(error) = fs::rename(staging.path().join(&skill.candidate.name), &destination) {
            rollback_install(&installed_destinations);
            return Err(io_error(error));
        }
        installed_destinations.push(destination);
    }
    Ok(installed_destinations)
}

fn rollback_install(destinations: &[PathBuf]) {
    for destination in destinations.iter().rev() {
        let _ = fs::remove_dir_all(destination);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, name: &str, marker: &str) {
        let directory = root.join(".codex/skills").join(name);
        fs::create_dir_all(&directory).expect("create skill directory");
        fs::write(
            directory.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test skill\n---\n\n{marker}\n"),
        )
        .expect("write skill");
    }

    fn write_install_candidate(root: &Path, path: &str, name: &str, description: &str) {
        let directory = root.join(path);
        fs::create_dir_all(&directory).expect("create install candidate");
        fs::write(
            directory.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n"),
        )
        .expect("write install candidate");
    }

    #[test]
    fn repository_discovery_returns_all_valid_skills_in_path_order() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repository = temp.path().join("repository");
        write_install_candidate(
            &repository,
            "skills/extension-test",
            "extension-test",
            "Test browser extensions",
        );
        write_install_candidate(
            &repository,
            "skills/extension-create",
            "extension-create",
            "Create browser extensions",
        );
        write_install_candidate(
            &repository,
            "skills/invalid",
            "Invalid Name",
            "Invalid skill name",
        );
        let installed_names = HashSet::from(["extension-test".to_owned()]);

        let discovery = discover_install_candidates(&repository, &repository, &installed_names)
            .expect("discover install candidates");

        assert_eq!(discovery.skipped_invalid, 1);
        assert_eq!(discovery.skills.len(), 2);
        assert_eq!(
            discovery
                .skills
                .iter()
                .map(|skill| skill.candidate.path.as_str())
                .collect::<Vec<_>>(),
            vec!["skills/extension-create", "skills/extension-test"]
        );
        assert!(!discovery.skills[0].candidate.installed);
        assert!(discovery.skills[1].candidate.installed);
    }

    #[test]
    fn batch_install_copies_selected_skills_and_source_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repository = temp.path().join("repository");
        let install_root = temp.path().join("home/.agents/skills");
        write_install_candidate(
            &repository,
            "skills/extension-create",
            "extension-create",
            "Create browser extensions",
        );
        write_install_candidate(
            &repository,
            "skills/extension-test",
            "extension-test",
            "Test browser extensions",
        );
        let discovery = discover_install_candidates(&repository, &repository, &HashSet::new())
            .expect("discover install candidates");
        let selected: Vec<_> = discovery.skills.iter().collect();

        let installed = install_candidate_directories(
            &install_root,
            &selected,
            "https://github.com/quangpl/browser-extension-skills",
        )
        .expect("install selected candidates");

        assert_eq!(installed.len(), 2);
        assert!(install_root.join("extension-create/SKILL.md").is_file());
        assert!(install_root.join("extension-test/SKILL.md").is_file());
        let source: Value = serde_json::from_slice(
            &fs::read(
                install_root
                    .join("extension-create")
                    .join(".cmdgpt-source.json"),
            )
            .expect("read source metadata"),
        )
        .expect("parse source metadata");
        assert_eq!(
            source.get("repositoryUrl").and_then(Value::as_str),
            Some("https://github.com/quangpl/browser-extension-skills")
        );
        assert_eq!(
            source.get("skillPath").and_then(Value::as_str),
            Some("skills/extension-create")
        );
    }

    #[test]
    fn root_skill_install_excludes_git_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repository = temp.path().join("repository");
        let install_root = temp.path().join("home/.agents/skills");
        write_install_candidate(
            &repository,
            "",
            "root-skill",
            "Skill stored at repository root",
        );
        let git_metadata = repository.join(".git/objects");
        fs::create_dir_all(&git_metadata).expect("create git metadata");
        fs::write(git_metadata.join("noise"), "not skill content").expect("write git metadata");
        let discovery = discover_install_candidates(&repository, &repository, &HashSet::new())
            .expect("discover root skill");
        let selected: Vec<_> = discovery.skills.iter().collect();

        install_candidate_directories(
            &install_root,
            &selected,
            "https://github.com/example/root-skill",
        )
        .expect("install root skill");

        assert!(install_root.join("root-skill/SKILL.md").is_file());
        assert!(!install_root.join("root-skill/.git").exists());
    }

    #[tokio::test]
    async fn workspace_can_switch_without_recreating_skill_service() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let startup = temp.path().join("startup");
        let project_a = temp.path().join("project-a");
        let project_b = temp.path().join("project-b");
        write_skill(&home, "shared-skill", "global");
        write_skill(&startup, "startup-skill", "startup");
        write_skill(&project_a, "shared-skill", "project-a");
        write_skill(&project_b, "shared-skill", "project-b");

        let service = SkillService::new(Some(&home), Some(&startup), None, 10_000);
        let from_a = service
            .read_for_workspace("shared-skill", Some(&project_a))
            .await
            .expect("read project a skill");
        let from_b = service
            .read_for_workspace("shared-skill", Some(&project_b))
            .await
            .expect("read project b skill");

        assert!(from_a.instructions.contains("project-a"));
        assert!(from_b.instructions.contains("project-b"));
        assert!(
            from_a
                .source
                .starts_with(project_a.to_string_lossy().as_ref())
        );
        assert!(
            from_b
                .source
                .starts_with(project_b.to_string_lossy().as_ref())
        );
    }

    #[tokio::test]
    async fn skill_read_validates_parameters_and_denies_path_traversal() {
        let temp = tempfile::tempdir().expect("tempdir");
        let global_skills = temp.path().join("skills/global");
        let skill_dir = global_skills.join("test-tool-skill");
        fs::create_dir_all(skill_dir.join("examples")).expect("create examples dir");

        fs::write(
            skill_dir.join("core.md"),
            "---\nname: test-tool-skill\ndescription: A test tool skill\n---\nCore instructions\n",
        )
        .expect("write core.md");

        fs::write(
            skill_dir.join("examples/fs_read.md"),
            "Example content for fs_read",
        )
        .expect("write example");

        let service = SkillService::new(None, None, Some(&global_skills), 10_000);

        // 1. Valid read core
        let core = service
            .read_for_task(None, "test-tool-skill", Some("core"), None, None)
            .await
            .expect("read core");
        assert!(core.instructions.contains("Core instructions"));

        // 2. Valid read examples list
        let examples_list = service
            .read_for_task(None, "test-tool-skill", Some("examples"), None, None)
            .await
            .expect("read examples list");
        assert!(examples_list.instructions.contains("fs_read"));

        // 3. Valid read specific example
        let tool_example = service
            .read_for_task(None, "test-tool-skill", Some("examples"), Some("fs_read"), None)
            .await
            .expect("read example");
        assert!(tool_example.instructions.contains("Example content for fs_read"));

        // 4. Reject ..\ traversal
        let err1 = service
            .read_for_task(None, "test-tool-skill", Some("examples"), Some(r"..\secret"), None)
            .await
            .unwrap_err();
        assert_eq!(err1.code, "invalid_tool_name");

        // 5. Reject ../ traversal
        let err2 = service
            .read_for_task(None, "test-tool-skill", Some("examples"), Some("../secret"), None)
            .await
            .unwrap_err();
        assert_eq!(err2.code, "invalid_tool_name");

        // 6. Reject absolute path (Windows style)
        let err3 = service
            .read_for_task(None, "test-tool-skill", Some("examples"), Some(r"C:\Windows\win.ini"), None)
            .await
            .unwrap_err();
        assert_eq!(err3.code, "invalid_tool_name");

        // 7. Reject absolute path (Unix style)
        let err4 = service
            .read_for_task(None, "test-tool-skill", Some("examples"), Some("/etc/passwd"), None)
            .await
            .unwrap_err();
        assert_eq!(err4.code, "invalid_tool_name");

        // 8. Reject empty tool string
        let err5 = service
            .read_for_task(None, "test-tool-skill", Some("examples"), Some(""), None)
            .await
            .unwrap_err();
        assert_eq!(err5.code, "invalid_tool_name");

        // 9. Reject uppercase letters
        let err6 = service
            .read_for_task(None, "test-tool-skill", Some("examples"), Some("FS_READ"), None)
            .await
            .unwrap_err();
        assert_eq!(err6.code, "invalid_tool_name");

        // 10. Reject invalid tier
        let err7 = service
            .read_for_task(None, "test-tool-skill", Some("resident"), None, None)
            .await
            .unwrap_err();
        assert_eq!(err7.code, "invalid_tier");

        // 11. Reject invalid skill_id with ..
        let err8 = service
            .read_for_task(None, "../test-tool-skill", None, None, None)
            .await
            .unwrap_err();
        assert_eq!(err8.code, "invalid_skill_id");

        // 12. Reject empty skill_id
        let err9 = service
            .read_for_task(None, "", None, None, None)
            .await
            .unwrap_err();
        assert_eq!(err9.code, "invalid_skill_id");
    }

    #[tokio::test]
    async fn global_skills_read_in_place_and_missing_dir_handled() {
        let temp = tempfile::tempdir().expect("tempdir");
        let global_skills = temp.path().join("skills/global");
        let skill_dir = global_skills.join("in-place-skill");
        fs::create_dir_all(skill_dir.join("examples")).expect("create dirs");

        fs::write(
            skill_dir.join("resident.md"),
            "Resident in place guidance",
        )
        .expect("write resident");

        fs::write(
            skill_dir.join("core.md"),
            "---\nname: in-place-skill\ndescription: In place test\n---\nCore instructions in place\n",
        )
        .expect("write core");

        fs::write(
            skill_dir.join("examples/run_test.md"),
            "Example run test",
        )
        .expect("write example");

        // Discovered from global_skills in-place
        let service = SkillService::new(None, None, Some(&global_skills), 10_000);
        let list = service.list().await.expect("list skills");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "in-place-skill");
        assert!(list[0].has_resident);
        assert!(list[0].has_core);
        assert_eq!(list[0].examples, vec!["run_test"]);

        let (resident_content, _) = service
            .active_resident_instructions_for_task(None, None)
            .await
            .expect("resident instructions");
        assert!(resident_content.contains("Resident in place guidance"));

        // Missing global directory should be treated as empty without error
        let missing_dir = temp.path().join("non_existent_skills");
        let service_missing = SkillService::new(None, None, Some(&missing_dir), 10_000);
        let list_empty = service_missing.list().await.expect("list missing dir");
        assert!(list_empty.is_empty());
    }

    #[tokio::test]
    async fn user_home_skills_do_not_use_resident() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let skill_dir = home.join(".agents/skills/home-skill");
        fs::create_dir_all(&skill_dir).expect("create home skill dir");

        fs::write(
            skill_dir.join("resident.md"),
            "Home resident instructions which must be ignored",
        )
        .expect("write resident");

        fs::write(
            skill_dir.join("core.md"),
            "---\nname: home-skill\ndescription: Home skill\n---\nHome core instructions\n",
        )
        .expect("write core");

        let empty_global = temp.path().join("global");
        let service = SkillService::new(Some(&home), None, Some(&empty_global), 10_000);
        let list = service.list().await.expect("list skills");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "home-skill");
        assert!(!list[0].has_resident); // resident must be false / ignored

        let (resident_content, warnings) = service
            .active_resident_instructions_for_task(None, None)
            .await
            .expect("resident instructions");
        assert!(!resident_content.contains("Home resident instructions"));
        assert!(warnings.iter().any(|w| w.contains("ignored")));
    }

    #[test]
    fn local_source_registration_and_validation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let empty_global = temp.path().join("global");
        let service = SkillService::new(Some(&home), None, Some(&empty_global), 10_000);

        // 1. Rejects path with ".."
        let err_dotdot = service.add_local_source("C:/foo/../bar", "global").unwrap_err();
        assert_eq!(err_dotdot.code, "invalid_local_source");

        // 2. Rejects non-existent directory
        let err_not_exist = service.add_local_source("C:/non/existent/path/never", "global").unwrap_err();
        assert_eq!(err_not_exist.code, "invalid_local_source");

        // 3. Rejects directory with no skills
        let empty_folder = temp.path().join("empty_folder");
        fs::create_dir_all(&empty_folder).expect("create empty folder");
        let err_no_skills = service.add_local_source(empty_folder.to_str().unwrap(), "global").unwrap_err();
        assert_eq!(err_no_skills.code, "invalid_local_source");

        // 4. Accepts valid folder with at least one skill
        let valid_source = temp.path().join("valid_skills");
        let skill_a = valid_source.join("skill-a");
        fs::create_dir_all(&skill_a).expect("create skill_a");
        fs::write(
            skill_a.join("core.md"),
            "---\nname: skill-a\ndescription: Test A\n---\nCore instructions A\n",
        )
        .expect("write core");

        let source = service.add_local_source(valid_source.to_str().unwrap(), "global").expect("add source");
        assert_eq!(source.scope, "global");
        assert!(!source.id.is_empty());

        // 5. Rejects duplicate registration
        let err_dup = service.add_local_source(valid_source.to_str().unwrap(), "global").unwrap_err();
        assert_eq!(err_dup.code, "local_source_conflict");

        // 6. Listed in list_local_sources
        let sources = service.list_local_sources().expect("list sources");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, source.id);
    }

    #[test]
    fn remove_local_source_only_deregisters_and_keeps_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let empty_global = temp.path().join("global");
        let service = SkillService::new(Some(&home), None, Some(&empty_global), 10_000);

        let source_folder = temp.path().join("my_skills");
        let skill = source_folder.join("skill-keep");
        fs::create_dir_all(&skill).expect("create skill");
        let core_file = skill.join("core.md");
        fs::write(
            &core_file,
            "---\nname: skill-keep\ndescription: Must keep\n---\nImportant instructions\n",
        )
        .expect("write core");

        let added = service.add_local_source(source_folder.to_str().unwrap(), "global").expect("add source");
        assert_eq!(service.list_local_sources().unwrap().len(), 1);

        // Remove source
        let removed = service.remove_local_source(&added.id).expect("remove source");
        assert!(removed);
        assert_eq!(service.list_local_sources().unwrap().len(), 0);

        // Crucial test: Files on disk must STILL exist!
        assert!(core_file.is_file(), "core.md must NOT be deleted upon source removal");
        assert!(skill.is_dir(), "skill directory must NOT be deleted");

        // Removing again returns false
        assert!(!service.remove_local_source(&added.id).expect("remove again"));
    }

    #[test]
    fn lint_local_source_reports_diagnostics_correctly() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let empty_global = temp.path().join("global");
        let service = SkillService::new(Some(&home), None, Some(&empty_global), 10_000);

        let source_folder = temp.path().join("lint_skills");
        let skill = source_folder.join("bad-resident-skill");
        fs::create_dir_all(&skill).expect("create skill");

        // Write a resident.md that exceeds 300 chars to trigger a lint Error
        let long_resident = "x".repeat(350);
        fs::write(skill.join("resident.md"), &long_resident).expect("write resident");
        // Use legacy SKILL.md to trigger a lint Warning
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: bad-resident-skill\ndescription: Bad resident\n---\nInstructions\n",
        )
        .expect("write SKILL.md");

        let added = service.add_local_source(source_folder.to_str().unwrap(), "global").expect("add");

        let lint_results = service.lint_local_source(&added.id).expect("lint source");
        assert_eq!(lint_results.len(), 1);
        let diags = &lint_results[0].diagnostics;

        // Must report resident_too_long Error and legacy_skill_format Warning
        assert!(diags.iter().any(|d| d.code == "resident_too_long" && d.severity == LintSeverity::Error));
        assert!(diags.iter().any(|d| d.code == "legacy_skill_format" && d.severity == LintSeverity::Warning));
    }

    #[tokio::test]
    async fn resident_usage_and_preview_work_accurately() {
        let temp = tempfile::tempdir().expect("tempdir");
        let global = temp.path().join("skills/global");
        let skill = global.join("preview-skill");
        fs::create_dir_all(&skill).expect("create skill");

        fs::write(skill.join("resident.md"), "Strict rule: do not lie.").expect("write resident");
        fs::write(
            skill.join("core.md"),
            "---\nname: preview-skill\ndescription: Test\n---\nCore\n",
        )
        .expect("write core");

        let service = SkillService::new(None, None, Some(&global), 10_000);

        // 1. resident_usage
        let usage = service.resident_usage(None, None).await.expect("usage");
        assert!(usage.used_chars > 0);
        assert_eq!(usage.limit_chars, 800);

        // 2. resident_preview_for_project
        let preview = service.resident_preview_for_project(None).await.expect("preview");
        assert!(preview.resident_instructions.contains("Strict rule: do not lie."));
        assert_eq!(preview.skill_names, vec!["preview-skill"]);
    }

    #[test]
    fn global_diagnostics_reports_skills_and_skipped_subdirectories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let global = temp.path().join("skills/global");

        // 1. Valid skill
        let valid_skill = global.join("valid-skill");
        fs::create_dir_all(&valid_skill).expect("create valid skill");
        fs::write(
            valid_skill.join("core.md"),
            "---\nname: valid-skill\ndescription: Valid\n---\nCore\n",
        )
        .expect("write core");

        // 2. Hidden directory
        let hidden_dir = global.join(".system_store");
        fs::create_dir_all(&hidden_dir).expect("create hidden dir");

        // 3. Missing SKILL.md/core.md
        let missing_md = global.join("empty-skill");
        fs::create_dir_all(&missing_md).expect("create empty dir");

        let service = SkillService::new(None, None, Some(&global), 10_000);
        let diags = service.global_diagnostics().expect("diagnostics");

        assert_eq!(diags.skill_count, 1);
        assert_eq!(diags.skipped_subdirectories.len(), 2);

        let skipped_names: Vec<_> = diags.skipped_subdirectories.iter().map(|s| s.name.as_str()).collect();
        assert!(skipped_names.contains(&".system_store"));
        assert!(skipped_names.contains(&"empty-skill"));
    }

    #[tokio::test]
    async fn reload_all_evicts_task_snapshots_and_rediscovers_skills() {
        let temp = tempfile::tempdir().expect("tempdir");
        let global = temp.path().join("skills/global");
        let skill = global.join("reloadable");
        fs::create_dir_all(&skill).expect("create skill");
        let core = skill.join("core.md");
        fs::write(&core, "---\nname: reloadable\ndescription: Reload test\n---\nold").expect("write old");
        let service = SkillService::new(None, None, Some(&global), 10_000);
        assert!(service.read_for_task(Some("task"), "reloadable", None, None, None).await.expect("read old").instructions.contains("old"));
        fs::write(&core, "---\nname: reloadable\ndescription: Reload test\n---\nnew").expect("write new");
        assert!(service.read_for_task(Some("task"), "reloadable", None, None, None).await.expect("cached read").instructions.contains("old"));
        assert_eq!(service.reload_all().await.expect("reload"), 1);
        assert!(service.read_for_task(Some("task"), "reloadable", None, None, None).await.expect("read new").instructions.contains("new"));
    }

    #[test]
    fn test_resolve_default_global_skills_dir_variations() {
        let repo_root = Path::new(r"D:\frank\gemini\ChatCmd");
        let target_release_workdir = Path::new(r"D:\frank\gemini\ChatCmd\target\release");
        let other_workdir = Path::new(r"C:\some\other\workspace");

        let workdirs: [Option<&Path>; 4] = [
            Some(repo_root),
            Some(target_release_workdir),
            Some(other_workdir),
            None,
        ];

        let expected_repo_skills = PathBuf::from(r"D:\frank\gemini\ChatCmd\skills\global");

        // 1. Target release exe
        let exe_release = Path::new(r"D:\frank\gemini\ChatCmd\target\release\chatcmd.exe");
        let exe_release_verbatim = Path::new(r"\\?\D:\frank\gemini\ChatCmd\target\release\chatcmd.exe");
        for &wd in &workdirs {
            let res = resolve_default_global_skills_dir_from(Some(exe_release), wd);
            assert_eq!(res, expected_repo_skills, "Failed for release exe with workdir {:?}", wd);
            let res_v = resolve_default_global_skills_dir_from(Some(exe_release_verbatim), wd);
            assert_eq!(res_v, expected_repo_skills, "Failed for verbatim release exe with workdir {:?}", wd);
            assert!(!res.to_string_lossy().starts_with(r"\\?\"));
            assert!(!res_v.to_string_lossy().starts_with(r"\\?\"));
        }

        // 2. Target debug exe
        let exe_debug = Path::new(r"D:\frank\gemini\ChatCmd\target\debug\chatcmd.exe");
        let exe_debug_verbatim = Path::new(r"\\?\D:\frank\gemini\ChatCmd\target\debug\chatcmd.exe");
        for &wd in &workdirs {
            let res = resolve_default_global_skills_dir_from(Some(exe_debug), wd);
            assert_eq!(res, expected_repo_skills, "Failed for debug exe with workdir {:?}", wd);
            let res_v = resolve_default_global_skills_dir_from(Some(exe_debug_verbatim), wd);
            assert_eq!(res_v, expected_repo_skills, "Failed for verbatim debug exe with workdir {:?}", wd);
            assert!(!res.to_string_lossy().starts_with(r"\\?\"));
            assert!(!res_v.to_string_lossy().starts_with(r"\\?\"));
        }

        // 3. Installed exe outside target
        let exe_installed = Path::new(r"C:\Program Files\ChatCmd\chatcmd.exe");
        let expected_installed_skills = PathBuf::from(r"C:\Program Files\ChatCmd\skills\global");
        for &wd in &workdirs {
            let res = resolve_default_global_skills_dir_from(Some(exe_installed), wd);
            assert_eq!(res, expected_installed_skills, "Failed for installed exe with workdir {:?}", wd);
            assert!(!res.to_string_lossy().starts_with(r"\\?\"));
        }
    }
}
