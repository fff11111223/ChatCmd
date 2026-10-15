use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use chatcmd_runtime::{OperationContext, RuntimeResult, ShellCreateRequest, ShellWriteRequest};
use serde_json::{Value, json};

use super::super::{
    RuntimeHost,
    inputs::{
        SessionInput, ShellClose, ShellCreate, ShellResize, ShellSignalInput, ShellWait, ShellWrite,
    },
    value,
};

impl RuntimeHost {
    pub(super) async fn dispatch_shell_create(
        &self,
        context: &OperationContext,
        input: ShellCreate,
        project_folder: Option<&Path>,
        task_path_scopes: &[PathBuf],
    ) -> RuntimeResult<Value> {
        self.retire_idle_current_turn_terminals(context).await?;
        let working_directory = match input.working_directory {
            Some(value) if value.is_absolute() => value,
            Some(value) => project_folder
                .map(|folder| folder.join(value))
                .ok_or_else(super::helpers::project_folder_required_for_shell)?,
            None => project_folder
                .map(Path::to_path_buf)
                .ok_or_else(super::helpers::project_folder_required_for_shell)?,
        };
        let mut shell_scopes = task_path_scopes.to_vec();
        if let Some(scope) = super::path_scopes::scope_for_path(&working_directory) {
            // Never widen sandbox scopes to an unrelated absolute working directory.
            let already_covered = shell_scopes
                .iter()
                .any(|allowed| scope.starts_with(allowed));
            if already_covered || project_folder.is_none() {
                shell_scopes.push(scope);
                shell_scopes.sort();
                shell_scopes.dedup();
            }
        }
        self.enable_shell_file_watcher(context);
        let info = self
            .shell
            .create_with_additional_scopes(
                context,
                ShellCreateRequest {
                    request_id: context.request_id.clone(),
                    working_directory: Some(working_directory),
                    executable: input.executable,
                    arguments: input.arguments,
                    environment: input.environment,
                    columns: input.columns,
                    rows: input.rows,
                },
                &shell_scopes,
            )
            .await?;
        self.persist_shell_session(context, &info).await?;
        self.publish_terminal_opened(context, &info);
        self.spawn_terminal_live_bridge(context, &info);
        value(info)
    }

    pub(super) async fn dispatch_shell_write(
        &self,
        context: &OperationContext,
        input: ShellWrite,
    ) -> RuntimeResult<Value> {
        let written = self
            .shell
            .write(
                context,
                ShellWriteRequest {
                    request_id: context.request_id.clone(),
                    session_id: input.session_id,
                    text: input.text,
                    append_new_line: input.append_new_line,
                    input_kind: input.input_kind,
                    sensitive: input.sensitive,
                },
            )
            .await?;
        Ok(json!({ "writtenBytes": written }))
    }

    pub(super) async fn dispatch_shell_signal(
        &self,
        context: &OperationContext,
        input: ShellSignalInput,
    ) -> RuntimeResult<Value> {
        self.shell
            .signal(context, &input.session_id, input.signal)
            .await?;
        Ok(json!({ "accepted": true }))
    }

    pub(super) async fn dispatch_shell_resize(
        &self,
        context: &OperationContext,
        input: ShellResize,
    ) -> RuntimeResult<Value> {
        value(
            self.shell
                .resize_with_context(context, &input.session_id, input.columns, input.rows)
                .await?,
        )
    }

    pub(super) async fn dispatch_shell_close(
        &self,
        context: &OperationContext,
        input: ShellClose,
    ) -> RuntimeResult<Value> {
        self.shell
            .close(context, &input.session_id, input.force)
            .await?;
        self.update_session_status(&input.session_id, "closed", None)
            .await?;
        Ok(json!({ "closed": true }))
    }

    pub(super) async fn dispatch_shell_list(&self) -> RuntimeResult<Value> {
        value(self.shell.list().await?)
    }

    pub(super) async fn dispatch_shell_inspect(&self, input: SessionInput) -> RuntimeResult<Value> {
        value(self.shell.inspect(&input.session_id).await?)
    }

    pub(super) async fn dispatch_shell_wait(
        &self,
        context: &OperationContext,
        input: ShellWait,
    ) -> RuntimeResult<Value> {
        let (result, usage) = self
            .wait_for_shell_handoff(
                context,
                &input.session_id,
                Duration::from_millis(input.timeout_ms.clamp(1, 300_000)),
                input.allow_user_input,
            )
            .await?;
        if result.completed {
            self.update_session_status(&input.session_id, "exited", result.exit_code)
                .await?;
        }
        super::helpers::value_with_usage(result, usage)
    }
}
