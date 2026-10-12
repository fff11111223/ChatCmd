use std::{collections::HashSet, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chatcmd_core::TaskId;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::websocket::AppState;

use super::{Problem, db_problem, iso_ms, now_ms, task_delete::delete_task_by_id};

const MAX_PROJECT_NAME_CHARS: usize = 160;
const MAX_PROJECT_PATH_CHARS: usize = 4_096;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveWorkspaceProject {
    name: String,
    path: Option<String>,
    paths: Option<Vec<String>>,
    #[serde(rename = "chatGptProjectUrl")]
    chatgpt_project_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReorderWorkspaceProjects {
    project_ids: Vec<String>,
}

pub(super) async fn workspace_projects(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, Problem> {
    let rows = sqlx::query(
        "SELECT id,name,path,canonical_path,paths,chatgpt_project_url,sort_order,created_at_ms,updated_at_ms FROM workspace_projects ORDER BY COALESCE(sort_order, 2147483647), updated_at_ms DESC, name COLLATE NOCASE",
    )
    .fetch_all(state.repository.pool())
    .await
    .map_err(db_problem)?;
    Ok(Json(Value::Array(
        rows.iter().map(workspace_project_value).collect(),
    )))
}

fn resolve_and_validate_project_paths(
    name: &str,
    input_path: Option<&str>,
    input_paths: Option<&[String]>,
) -> Result<Vec<String>, Problem> {
    if name.is_empty() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid workspace project",
            "name is required",
        ));
    }
    if name.chars().count() > MAX_PROJECT_NAME_CHARS {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid workspace project",
            "name is too long",
        ));
    }

    let mut resolved = Vec::new();
    if let Some(list) = input_paths {
        for p in list {
            let trimmed = p.trim();
            if !trimmed.is_empty()
                && !resolved
                    .iter()
                    .any(|existing: &String| canonical_project_path(existing) == canonical_project_path(trimmed))
            {
                resolved.push(trimmed.to_string());
            }
        }
    }
    if resolved.is_empty() {
        if let Some(p) = input_path {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
                resolved.push(trimmed.to_string());
            }
        }
    }

    if resolved.is_empty() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid workspace project",
            "at least one project folder path is required",
        ));
    }

    for p in &resolved {
        if p.chars().count() > MAX_PROJECT_PATH_CHARS {
            return Err(Problem::new(
                StatusCode::BAD_REQUEST,
                "Invalid workspace project",
                "path is too long",
            ));
        }
    }

    Ok(resolved)
}

fn check_project_path_conflicts(
    rows: &[sqlx::sqlite::SqliteRow],
    target_canonical_paths: &[String],
    exclude_project_id: Option<&str>,
) -> Result<(), Problem> {
    for row in rows {
        let id = row.get::<String, _>("id");
        if let Some(exclude) = exclude_project_id {
            if id == exclude {
                continue;
            }
        }
        let project_canonical = row.get::<String, _>("canonical_path");
        let project_paths: Vec<String> = row
            .try_get::<Option<String>, _>("paths")
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_else(|| vec![row.get::<String, _>("path")]);

        for target in target_canonical_paths {
            if *target == project_canonical
                || project_paths
                    .iter()
                    .any(|p| canonical_project_path(p) == *target)
            {
                return Err(Problem::new(
                    StatusCode::CONFLICT,
                    "Workspace project already exists",
                    "This folder is already used by another project.",
                ));
            }
        }
    }
    Ok(())
}

pub(super) async fn save_workspace_project(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SaveWorkspaceProject>,
) -> Result<Json<Value>, Problem> {
    let name = input.name.trim();
    let paths = resolve_and_validate_project_paths(name, input.path.as_deref(), input.paths.as_deref())?;
    let chatgpt_project_url = normalize_chatgpt_project_url(input.chatgpt_project_url.as_deref())?;

    let existing_rows = sqlx::query("SELECT id,path,canonical_path,paths FROM workspace_projects")
        .fetch_all(state.repository.pool())
        .await
        .map_err(db_problem)?;
    let canonical_paths: Vec<String> = paths.iter().map(|p| canonical_project_path(p)).collect();
    check_project_path_conflicts(&existing_rows, &canonical_paths, None)?;

    let primary_path = &paths[0];
    let primary_canonical = &canonical_paths[0];
    let paths_json = serde_json::to_string(&paths).unwrap_or_default();
    let id = format!("project-{}", Uuid::new_v4());
    let now = now_ms();
    sqlx::query(
        "INSERT INTO workspace_projects(id,name,path,canonical_path,paths,chatgpt_project_url,sort_order,created_at_ms,updated_at_ms) VALUES(?,?,?,?,?,?,(SELECT COALESCE(MAX(sort_order),-1)+1 FROM workspace_projects),?,?) ON CONFLICT(canonical_path) DO UPDATE SET name=excluded.name,path=excluded.path,paths=excluded.paths,chatgpt_project_url=excluded.chatgpt_project_url,updated_at_ms=excluded.updated_at_ms",
    )
    .bind(&id)
    .bind(name)
    .bind(primary_path)
    .bind(primary_canonical)
    .bind(&paths_json)
    .bind(chatgpt_project_url.as_deref())
    .bind(now)
    .bind(now)
    .execute(state.repository.pool())
    .await
    .map_err(db_problem)?;
    let row = sqlx::query(
        "SELECT id,name,path,canonical_path,paths,chatgpt_project_url,created_at_ms,updated_at_ms FROM workspace_projects WHERE canonical_path=?",
    )
    .bind(primary_canonical)
    .fetch_one(state.repository.pool())
    .await
    .map_err(db_problem)?;
    Ok(Json(workspace_project_value(&row)))
}

pub(super) async fn update_workspace_project(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<SaveWorkspaceProject>,
) -> Result<Json<Value>, Problem> {
    let name = input.name.trim();
    let paths = resolve_and_validate_project_paths(name, input.path.as_deref(), input.paths.as_deref())?;
    let chatgpt_project_url = normalize_chatgpt_project_url(input.chatgpt_project_url.as_deref())?;

    let existing = sqlx::query("SELECT id,path,canonical_path,paths FROM workspace_projects WHERE id=?")
        .bind(&id)
        .fetch_optional(state.repository.pool())
        .await
        .map_err(db_problem)?
        .ok_or_else(|| {
            Problem::new(
                StatusCode::NOT_FOUND,
                "Workspace project not found",
                "Project no longer exists.",
            )
        })?;
    let old_canonical = existing.get::<String, _>("canonical_path");
    let old_paths: Vec<String> = existing
        .try_get::<Option<String>, _>("paths")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| vec![existing.get::<String, _>("path")]);
    let old_canonical_paths: Vec<String> = old_paths.iter().map(|p| canonical_project_path(p)).collect();

    let all_rows = sqlx::query("SELECT id,path,canonical_path,paths FROM workspace_projects")
        .fetch_all(state.repository.pool())
        .await
        .map_err(db_problem)?;
    let canonical_paths: Vec<String> = paths.iter().map(|p| canonical_project_path(p)).collect();
    check_project_path_conflicts(&all_rows, &canonical_paths, Some(&id))?;

    let primary_path = &paths[0];
    let primary_canonical = &canonical_paths[0];
    let paths_json = serde_json::to_string(&paths).unwrap_or_default();

    let now = now_ms();
    let mut transaction = state.repository.pool().begin().await.map_err(db_problem)?;
    sqlx::query(
        "UPDATE workspace_projects SET name=?,path=?,canonical_path=?,paths=?,chatgpt_project_url=?,updated_at_ms=? WHERE id=?",
    )
    .bind(name)
    .bind(primary_path)
    .bind(primary_canonical)
    .bind(&paths_json)
    .bind(chatgpt_project_url.as_deref())
    .bind(now)
    .bind(&id)
    .execute(&mut *transaction)
    .await
    .map_err(db_problem)?;

    if old_canonical != *primary_canonical {
        let task_rows =
            sqlx::query("SELECT id,project_folder FROM tasks WHERE project_folder IS NOT NULL")
                .fetch_all(&mut *transaction)
                .await
                .map_err(db_problem)?;
        for row in task_rows {
            let folder = row.get::<String, _>("project_folder");
            if old_canonical_paths.contains(&canonical_project_path(&folder)) {
                sqlx::query("UPDATE tasks SET project_folder=?,updated_at_ms=? WHERE id=?")
                    .bind(primary_path)
                    .bind(now)
                    .bind(row.get::<String, _>("id"))
                    .execute(&mut *transaction)
                    .await
                    .map_err(db_problem)?;
            }
        }
        let request_rows = sqlx::query("SELECT id,project_folder FROM chatgpt_bridge_requests WHERE project_folder IS NOT NULL")
            .fetch_all(&mut *transaction)
            .await
            .map_err(db_problem)?;
        for row in request_rows {
            let folder = row.get::<String, _>("project_folder");
            if old_canonical_paths.contains(&canonical_project_path(&folder)) {
                sqlx::query("UPDATE chatgpt_bridge_requests SET project_folder=?,updated_at_ms=? WHERE id=?")
                    .bind(primary_path)
                    .bind(now)
                    .bind(row.get::<String, _>("id"))
                    .execute(&mut *transaction)
                    .await
                    .map_err(db_problem)?;
            }
        }
    }
    transaction.commit().await.map_err(db_problem)?;

    let row = sqlx::query(
        "SELECT id,name,path,canonical_path,paths,chatgpt_project_url,created_at_ms,updated_at_ms FROM workspace_projects WHERE id=?",
    )
    .bind(&id)
    .fetch_one(state.repository.pool())
    .await
    .map_err(db_problem)?;
    Ok(Json(workspace_project_value(&row)))
}

pub(super) async fn delete_workspace_project(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Problem> {
    let row = sqlx::query("SELECT id,path,canonical_path,paths FROM workspace_projects WHERE id=?")
        .bind(&id)
        .fetch_optional(state.repository.pool())
        .await
        .map_err(db_problem)?
        .ok_or_else(|| {
            Problem::new(
                StatusCode::NOT_FOUND,
                "Workspace project not found",
                "Project no longer exists.",
            )
        })?;
    let project_paths: Vec<String> = row
        .try_get::<Option<String>, _>("paths")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| vec![row.get::<String, _>("path")]);
    let project_canonical_paths: Vec<String> = project_paths
        .iter()
        .map(|p| canonical_project_path(p))
        .collect();

    let child_ids = sqlx::query_scalar::<_, String>(
        "SELECT child_task_id FROM subagent_runs WHERE child_task_id IS NOT NULL",
    )
    .fetch_all(state.repository.pool())
    .await
    .map_err(db_problem)?
    .into_iter()
    .collect::<HashSet<_>>();
    let task_rows =
        sqlx::query("SELECT id,status,project_folder FROM tasks WHERE project_folder IS NOT NULL")
            .fetch_all(state.repository.pool())
            .await
            .map_err(db_problem)?;

    let mut deleted = 0usize;
    let mut preserved = 0usize;
    for task_row in task_rows {
        let folder = task_row.get::<String, _>("project_folder");
        if !project_canonical_paths.contains(&canonical_project_path(&folder)) {
            continue;
        }
        let task_id = task_row.get::<String, _>("id");
        let status = task_row.get::<String, _>("status");
        if child_ids.contains(&task_id) {
            continue;
        }
        if matches!(status.as_str(), "pending" | "running")
            || has_active_descendant(&state, &task_id).await?
        {
            preserved += 1;
            continue;
        }
        let parsed = TaskId::new(&task_id).map_err(|_| {
            Problem::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid stored task id",
                "Unable to delete the project conversation.",
            )
        })?;
        delete_task_by_id(&state, &parsed).await?;
        deleted += 1;
    }

    sqlx::query("DELETE FROM workspace_projects WHERE id=?")
        .bind(&id)
        .execute(state.repository.pool())
        .await
        .map_err(db_problem)?;
    Ok(Json(
        json!({ "deleted": true, "deletedConversations": deleted, "preservedConversations": preserved }),
    ))
}

async fn has_active_descendant(state: &Arc<AppState>, task_id: &str) -> Result<bool, Problem> {
    let count = sqlx::query_scalar::<_, i64>(
        "WITH RECURSIVE descendants(id) AS (\
            SELECT child_task_id FROM subagent_runs WHERE parent_task_id=? AND child_task_id IS NOT NULL \
            UNION \
            SELECT r.child_task_id FROM subagent_runs r JOIN descendants d ON r.parent_task_id=d.id WHERE r.child_task_id IS NOT NULL\
         ) SELECT COUNT(*) FROM tasks WHERE id IN (SELECT id FROM descendants) AND status IN ('pending','running')",
    )
    .bind(task_id)
    .fetch_one(state.repository.pool())
    .await
    .map_err(db_problem)?;
    Ok(count > 0)
}


fn normalize_chatgpt_project_url(value: Option<&str>) -> Result<Option<String>, Problem> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    const PREFIX: &str = "https://chatgpt.com/g/g-p-";
    const SUFFIX: &str = "/project";
    let code = value
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
        .filter(|code| {
            !code.is_empty()
                && code
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        });
    if code.is_none() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid ChatGPT project link",
            "The ChatGPT project link must have the form [https://chatgpt.com/g/g-p-{ID}/project.](https://chatgpt.com/g/g-p-{ID}/project.)",
        ));
    }
    Ok(Some(value.to_owned()))
}

pub(super) async fn reorder_workspace_projects(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ReorderWorkspaceProjects>,
) -> Result<StatusCode, Problem> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_projects")
        .fetch_one(state.repository.pool())
        .await
        .map_err(db_problem)?;
    let unique_ids = input.project_ids.iter().collect::<HashSet<_>>();
    if input.project_ids.len() as i64 != count || unique_ids.len() != input.project_ids.len() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid workspace project order",
            "projectIds must contain every workspace project exactly once",
        ));
    }
    let mut transaction = state.repository.pool().begin().await.map_err(db_problem)?;
    for (position, project_id) in input.project_ids.iter().enumerate() {
        let result = sqlx::query("UPDATE workspace_projects SET sort_order=? WHERE id=?")
            .bind(position as i64)
            .bind(project_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_problem)?;
        if result.rows_affected() != 1 {
            return Err(Problem::new(
                StatusCode::BAD_REQUEST,
                "Invalid workspace project order",
                "projectIds contains an unknown or duplicate project",
            ));
        }
    }
    transaction.commit().await.map_err(db_problem)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn canonical_project_path(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    while value.len() > 1 && value.ends_with('/') {
        value.pop();
    }
    let bytes = value.as_bytes();
    let is_windows_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if is_windows_drive || value.starts_with("//") {
        value.make_ascii_lowercase();
    }
    value
}

fn workspace_project_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    let primary_path = row.get::<String, _>("path");
    let paths: Vec<String> = row
        .try_get::<Option<String>, _>("paths")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| vec![primary_path.clone()]);
    json!({
        "id": row.get::<String, _>("id"),
        "name": row.get::<String, _>("name"),
        "path": primary_path,
        "paths": paths,
        "chatGptProjectUrl": row.get::<Option<String>, _>("chatgpt_project_url"),
        "createdAtUtc": iso_ms(row.get::<i64, _>("created_at_ms")),
        "updatedAtUtc": iso_ms(row.get::<i64, _>("updated_at_ms"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_windows_workspace_folder_normalizes_identity() {
        assert_eq!(
            canonical_project_path(r" D:\DEV\Dotty\ "),
            canonical_project_path("d:/dev/dotty")
        );
    }

    #[test]
    fn canonical_posix_workspace_folder_preserves_case() {
        assert_eq!(canonical_project_path(" /Work/Client/ "), "/Work/Client");
        assert_ne!(
            canonical_project_path("/Work/Client"),
            canonical_project_path("/work/client")
        );
    }

    #[test]
    fn chatgpt_project_url_requires_project_shape() {
        assert_eq!(
            normalize_chatgpt_project_url(Some(" https://chatgpt.com/g/g-p-demo_123/project "))
                .expect("valid project url"),
            Some("https://chatgpt.com/g/g-p-demo_123/project".to_owned())
        );
        assert!(normalize_chatgpt_project_url(Some("https://chatgpt.com/")).is_err());
        assert!(
            normalize_chatgpt_project_url(Some("https://chatgpt.com/g/g-p-demo/project?x=1"))
                .is_err()
        );
        assert_eq!(
            normalize_chatgpt_project_url(Some("  ")).expect("empty optional url"),
            None
        );
    }
}
