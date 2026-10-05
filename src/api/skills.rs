use super::*;
use chatcmd_runtime::{ManagedSkill, RuntimeError, SkillLintDiagnostic};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SetSkillEnabled {
    enabled: Option<bool>,
    is_enabled: Option<bool>,
}
#[derive(Deserialize)]
pub(super) struct SetSkillOptions {
    #[serde(default)]
    options: HashMap<String, Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstallSkill {
    repository_url: String,
    #[serde(default)]
    skill_paths: Vec<String>,
}

// ── Local source management request bodies ────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AddLocalSource {
    /// Absolute path to the folder.
    path: String,
    /// "global" or a project name.
    #[serde(default = "default_global_scope")]
    scope: String,
}
fn default_global_scope() -> String {
    "global".into()
}

// ── Existing CRUD endpoints ───────────────────────────────────────────────

pub(super) async fn skills(State(state): State<Arc<AppState>>) -> Result<Json<Value>, Problem> {
    let values = state.skills.list_global().await.map_err(runtime_problem)?;
    let items = values
        .into_iter()
        .map(|skill| managed_skill_value(&skill))
        .collect();
    Ok(Json(Value::Array(items)))
}

pub(super) async fn skill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Problem> {
    let skill = state.skills.read(&id).await.map_err(runtime_problem)?;
    Ok(Json(
        json!({ "id": skill.id, "name": skill.name, "source": skill.source, "enabled": true, "shadowed": false, "content": skill.instructions }),
    ))
}

pub(super) async fn set_skill_enabled(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<SetSkillEnabled>,
) -> Result<Json<Value>, Problem> {
    let enabled = input.enabled.or(input.is_enabled).ok_or_else(|| {
        Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid skill setting",
            "enabled is required",
        )
    })?;
    let skill = state
        .skills
        .set_enabled(&id, enabled)
        .await
        .map_err(runtime_problem)?
        .ok_or_else(|| {
            Problem::new(
                StatusCode::NOT_FOUND,
                "Skill not found",
                "The requested skill is unavailable.",
            )
        })?;
    Ok(Json(managed_skill_value(&skill)))
}

pub(super) async fn set_skill_options(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<SetSkillOptions>,
) -> Result<Json<Value>, Problem> {
    if input.options.len() > 50 {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Too many options",
            "A skill accepts at most 50 option values.",
        ));
    }
    let skill = state
        .skills
        .set_options(&id, input.options)
        .await
        .map_err(runtime_problem)?
        .ok_or_else(|| {
            Problem::new(
                StatusCode::NOT_FOUND,
                "Skill not found",
                "The requested skill is unavailable.",
            )
        })?;
    Ok(Json(managed_skill_value(&skill)))
}

pub(super) async fn preview_skills(
    State(state): State<Arc<AppState>>,
    Json(input): Json<InstallSkill>,
) -> Result<Json<Value>, Problem> {
    validate_repository_url(&input.repository_url)?;
    let preview = state
        .skills
        .preview_install(input.repository_url.trim())
        .await
        .map_err(skill_install_problem)?;
    Ok(Json(serde_json::to_value(preview).map_err(|error| {
        Problem::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Skill preview failed",
            error.to_string(),
        )
    })?))
}

pub(super) async fn install_skill(
    State(state): State<Arc<AppState>>,
    Json(input): Json<InstallSkill>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    validate_repository_url(&input.repository_url)?;
    let skills = state
        .skills
        .install(input.repository_url.trim(), &input.skill_paths)
        .await
        .map_err(skill_install_problem)?;
    let values: Vec<_> = skills.iter().map(managed_skill_value).collect();
    Ok((StatusCode::CREATED, Json(json!({ "skills": values }))))
}

fn validate_repository_url(repository_url: &str) -> Result<(), Problem> {
    if repository_url.len() > 2048 {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid repository",
            "repositoryUrl is too long.",
        ));
    }
    Ok(())
}

fn skill_install_problem(error: RuntimeError) -> Problem {
    let status = if error.code == "skill_conflict" {
        StatusCode::CONFLICT
    } else {
        StatusCode::BAD_REQUEST
    };
    Problem::new(
        status,
        if status == StatusCode::CONFLICT {
            "Skill already installed"
        } else {
            "Skill installation failed"
        },
        error.message,
    )
}

/// Serialise a `ManagedSkill` to the JSON shape returned by the API.
/// New fields (sourceType, hasResident, etc.) are included when present;
/// callers that don't know about them simply ignore the extra keys.
fn managed_skill_value(skill: &ManagedSkill) -> Value {
    let icon_url = skill
        .icon_path
        .as_ref()
        .map(|_| format!("/api/local/skills/{}/icon", skill.id));
    json!({
        "id": skill.id,
        "title": skill.title,
        "description": skill.description,
        "iconUrl": icon_url,
        "source": skill.source,
        "sourceUrl": skill.source_url,
        "enabled": skill.enabled,
        "canDelete": skill.can_delete,
        "options": skill.options,
        // Extended fields — present only when non-default so old clients ignore them cleanly.
        "sourceType": skill.source_type,
        "hasResident": skill.has_resident,
        "residentChars": skill.resident_chars,
        "hasCore": skill.has_core,
        "examples": skill.examples,
        "overrides": skill.overrides,
        "warnings": skill.warnings,
        "errors": skill.errors,
    })
}

pub(super) async fn delete_skill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, Problem> {
    if state.skills.delete(&id).await.map_err(runtime_problem)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Skill not found",
            "The requested skill is unavailable.",
        ))
    }
}

pub(super) async fn skill_icon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, Problem> {
    let Some((path, content_type)) = state.skills.icon(&id).await.map_err(runtime_problem)? else {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Skill icon not found",
            "The requested skill does not expose an icon.",
        ));
    };
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        Problem::new(
            StatusCode::NOT_FOUND,
            "Skill icon not found",
            error.to_string(),
        )
    })?;
    Ok(([(header::CONTENT_TYPE, content_type)], bytes).into_response())
}

// ── Local source management endpoints ────────────────────────────────────

/// GET /skills/sources — list registered local folder sources.
pub(super) async fn list_local_sources(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, Problem> {
    let sources = state
        .skills
        .list_local_sources()
        .map_err(runtime_problem)?;
    Ok(Json(
        serde_json::to_value(&sources).unwrap_or(Value::Array(vec![])),
    ))
}

/// POST /skills/sources — register a new local folder source.
pub(super) async fn add_local_source(
    State(state): State<Arc<AppState>>,
    Json(input): Json<AddLocalSource>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let source = state
        .skills
        .add_local_source(input.path.trim(), input.scope.trim())
        .map_err(|e| {
            let status = if e.code == "local_source_conflict" {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            Problem::new(status, "Local source error", e.message)
        })?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&source).unwrap_or(Value::Null)),
    ))
}

/// DELETE /skills/sources/:source_id — deregister a local source (does not delete files).
pub(super) async fn remove_local_source(
    State(state): State<Arc<AppState>>,
    Path(source_id): Path<String>,
) -> Result<StatusCode, Problem> {
    if state
        .skills
        .remove_local_source(&source_id)
        .map_err(runtime_problem)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Source not found",
            "No local source with that id.",
        ))
    }
}

/// POST /skills/sources/:source_id/reload — clear snapshot cache and re-scan.
pub(super) async fn reload_local_source(
    State(state): State<Arc<AppState>>,
    Path(source_id): Path<String>,
) -> Result<StatusCode, Problem> {
    if state
        .skills
        .reload_local_source(&source_id)
        .await
        .map_err(runtime_problem)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Source not found",
            "No local source with that id.",
        ))
    }
}

/// GET /skills/sources/:source_id/lint — run lint on all skills in a local source.
pub(super) async fn lint_local_source(
    State(state): State<Arc<AppState>>,
    Path(source_id): Path<String>,
) -> Result<Json<Value>, Problem> {
    let results = state
        .skills
        .lint_local_source(&source_id)
        .map_err(|e| {
            let status = if e.code == "local_source_not_found" {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            };
            Problem::new(status, "Lint error", e.message)
        })?;
    let items: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "skillName": r.skill_name,
                "path": r.path,
                "diagnostics": lint_diagnostics_value(&r.diagnostics),
            })
        })
        .collect();
    Ok(Json(Value::Array(items)))
}

fn lint_diagnostics_value(diags: &[SkillLintDiagnostic]) -> Value {
    let items: Vec<Value> = diags
        .iter()
        .map(|d| {
            json!({
                "severity": format!("{:?}", d.severity).to_lowercase(),
                "code": d.code,
                "message": d.message,
                "path": d.path,
            })
        })
        .collect();
    Value::Array(items)
}

// ── Resident instruction endpoints ────────────────────────────────────────

/// GET /skills/resident-usage — character usage of resident instructions.
pub(super) async fn resident_usage(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, Problem> {
    let usage = state
        .skills
        .resident_usage(None, None)
        .await
        .map_err(runtime_problem)?;
    Ok(Json(
        serde_json::to_value(&usage).unwrap_or(Value::Null),
    ))
}

/// GET /skills/resident-preview — preview resident text the model would receive.
///
/// Accepts an optional `projectFolder` query parameter.
pub(super) async fn resident_preview(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, Problem> {
    let project_folder: Option<PathBuf> = params
        .get("projectFolder")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let preview = state
        .skills
        .resident_preview_for_project(project_folder.as_deref())
        .await
        .map_err(runtime_problem)?;
    Ok(Json(
        serde_json::to_value(&preview).unwrap_or(Value::Null),
    ))
}

/// GET /skills/diagnostics — read-only diagnostics of the global skills directory.
pub(super) async fn skill_diagnostics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, Problem> {
    let diags = state
        .skills
        .global_diagnostics()
        .map_err(runtime_problem)?;
    Ok(Json(
        serde_json::to_value(&diags).unwrap_or(Value::Null),
    ))
}

