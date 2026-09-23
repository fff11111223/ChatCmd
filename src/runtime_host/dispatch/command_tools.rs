use std::path::{Path, PathBuf};

use chatcmd_runtime::{CommandRunRequest, OperationContext, RuntimeError, RuntimeResult};
use serde_json::Value;

use super::super::{RuntimeHost, parse, value};
use super::path_scopes;

impl RuntimeHost {
    pub(super) async fn dispatch_command_run(
        &self,
        context: &OperationContext,
        arguments: Value,
        project_folder: Option<&Path>,
        task_path_scopes: &[PathBuf],
    ) -> RuntimeResult<Value> {
        let mut input: CommandRunRequest = parse(arguments)?;
        if input.cwd.is_relative() {
            input.cwd = project_folder
                .map(|folder| folder.join(&input.cwd))
                .ok_or_else(project_folder_required)?;
        }
        let mut scopes = task_path_scopes.to_vec();
        // Only add the cwd as an extra scope when it is already within an
        // existing allowed scope. Adding any absolute cwd unconditionally is a
        // sandbox bypass — the AI could set cwd to any path (e.g. C:\Users\frank)
        // and gain shell access to that directory.
        // If the safety_filter already validated cwd against project_folder
        // (which it does when enforce_project_folder_only is active), the cwd
        // will be inside task_path_scopes; scope_for_path is then a no-op.
        // When the sandbox is off, we keep the original behaviour and add any scope.
        if let Some(scope) = path_scopes::scope_for_path(&input.cwd) {
            let already_covered = scopes.iter().any(|s| scope.starts_with(s));
            if already_covered || project_folder.is_none() {
                // Inside allowed area, or no sandbox — add normally.
                scopes.push(scope);
                scopes.sort();
                scopes.dedup();
            }
            // else: cwd is outside all current scopes. The safety_filter should have
            // already rejected this call. If somehow we reach here, we simply don't
            // extend the scope — WorkspaceService.ensure_allowed will reject the access.
        }
        let workspace = self.workspace.with_additional_scopes(&scopes)?;
        let command = self.command.with_workspace(workspace);
        value(command.run(context, input).await?)
    }
}

fn project_folder_required() -> RuntimeError {
    RuntimeError::new(
        "project_folder_required",
        "relative command cwd requires the task project folder; otherwise provide an absolute cwd",
    )
}
