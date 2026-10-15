use std::{
    path::{Component, Path},
    time::Instant,
};

mod artifact_tools;
mod command_tools;
mod filesystem_tools;
mod helpers;
mod path_scopes;
mod shell_actions;
mod shell_handoff;
mod tool_authorization;

use chatcmd_core::{
    Artifact, ArtifactId, ArtifactStore, ExecutionMode, TaskExecutionMode, TaskId, TaskStore,
};
use chatcmd_mcp::RuntimeApi as _;
use chatcmd_runtime::{
    CommandOutput, FsConflictPolicy, FsTransferRequest, OperationContext, RuntimeError,
    RuntimeResult, ToolUsage,
};
use serde_json::{Value, json};

use super::turn_file_changes::{FileChangeKind, capture_snapshot};
use super::{
    RuntimeHost, filesystem_dispatch, inputs::*, invalid, now_ms, parse, storage_error, task_json,
    value,
};

impl RuntimeHost {
    pub(super) async fn dispatch(
        &self,
        tool: &str,
        context: OperationContext,
        arguments: Value,
    ) -> RuntimeResult<Value> {
        let filesystem_tool = is_filesystem_tool(tool);
        let (primary_project_folder, all_project_folders) = if filesystem_tool
            || tool.starts_with("git_")
            || tool == "shell_create"
            || tool == "shell_write"  // needed so command text can be scanned for out-of-scope paths
            || tool == "command_run"
            || tool == "workspace_roots"
            || tool == "project_context"
            || matches!(tool, "skills_list" | "skill_read")
        {
            let folders = self
                .task_project_folders(context.task_id.as_deref())
                .await?;
            let primary = folders.first().cloned();
            (primary, folders)
        } else {
            (None, Vec::new())
        };
        let project_folder = primary_project_folder.clone();
        let mut task_path_scopes = if filesystem_tool
            || tool.starts_with("git_")
            || matches!(tool, "command_run" | "shell_create" | "workspace_roots")
        {
            let raw_scopes = self.task_user_path_scopes(&context).await?;
            // When sandbox is active, filter user-mentioned paths to only those
            // within the project_folders. Without this, the AI gains access to ANY
            // absolute path the user ever mentioned in chat (e.g. "read C:\Users\frank\secret.txt").
            if self.load_safety_settings_enforce_only().await {
                if !all_project_folders.is_empty() {
                    let canonical_project_dirs: Vec<std::path::PathBuf> = all_project_folders
                        .iter()
                        .map(|f| {
                            super::safety_filter::strip_verbatim_prefix(
                                &f.canonicalize().unwrap_or_else(|_| f.clone()),
                            )
                        })
                        .collect();
                    raw_scopes
                        .into_iter()
                        .filter(|p| {
                            let p_canon = super::safety_filter::strip_verbatim_prefix(
                                &p.canonicalize().unwrap_or_else(|_| p.clone()),
                            );
                            canonical_project_dirs
                                .iter()
                                .any(|cf| super::safety_filter::is_subpath(&p_canon, cf))
                        })
                        .collect()
                } else {
                    // No project folder — don't grant any extra scopes from user messages.
                    Vec::new()
                }
            } else {
                raw_scopes
            }
        } else {
            Vec::new()
        };
        for folder in &all_project_folders {
            if folder.is_dir() && !task_path_scopes.contains(folder) {
                task_path_scopes.push(folder.clone());
            }
        }
        let arguments = if filesystem_tool {
            filesystem_dispatch::resolve_relative_paths(arguments, project_folder.as_deref())?
        } else {
            arguments
        };
        self.check_safety_filter(tool, &context, &arguments, &all_project_folders)
            .await?;
        if filesystem_tool || tool.starts_with("git_") {
            // Only add argument-derived scopes that are within the project folders when
            // the sandbox is active. Blindly adding any absolute path from tool arguments
            // as an allowed scope is a bypass vector — the AI can simply put an arbitrary
            // path in the arguments to gain filesystem access to it.
            let extra_scopes = path_scopes::argument_path_scopes(&arguments);
            let settings_enforce = self.load_safety_settings_enforce_only().await;
            for scope in extra_scopes {
                let allowed = if settings_enforce {
                    // Sandbox on: only allow scopes that are inside any project folder.
                    let canonical_project_dirs: Vec<std::path::PathBuf> = all_project_folders
                        .iter()
                        .map(|f| {
                            super::safety_filter::strip_verbatim_prefix(
                                &f.canonicalize().unwrap_or_else(|_| f.clone()),
                            )
                        })
                        .collect();
                    let scope_canon = super::safety_filter::strip_verbatim_prefix(
                        &scope.canonicalize().unwrap_or_else(|_| scope.clone()),
                    );
                    canonical_project_dirs
                        .iter()
                        .any(|cf| super::safety_filter::is_subpath(&scope_canon, cf))
                } else {
                    // Sandbox off: allow any scope (original behaviour).
                    true
                };
                if allowed && !task_path_scopes.contains(&scope) {
                    task_path_scopes.push(scope);
                }
            }
            task_path_scopes.sort();
            task_path_scopes.dedup();
        }
        let scoped_workspace = if filesystem_tool || tool.starts_with("git_") {
            Some(self.workspace.with_additional_scopes(&task_path_scopes)?)
        } else {
            None
        };
        let workspace = scoped_workspace.as_ref().unwrap_or(&self.workspace);
        let scoped_git = scoped_workspace
            .clone()
            .map(|workspace| self.git.with_workspace(workspace));
        let git = scoped_git.as_ref().unwrap_or(&self.git);

        if is_filesystem_tool(tool) {
            return self
                .dispatch_filesystem_tool(tool, &context, arguments, workspace)
                .await;
        }

        match tool {
            "device_list" => value(vec![self.local_device()]),
            "device_get" => {
                let input: DeviceGet = parse(arguments)?;
                if input.device_id != self.device.id.as_str() && input.device_id != "local" {
                    return Err(RuntimeError::new(
                        "device_not_found",
                        "device was not found",
                    ));
                }
                value(self.local_device())
            }
            "shell_create" => {
                let input: ShellCreate = parse(arguments)?;
                self.dispatch_shell_create(
                    &context,
                    input,
                    project_folder.as_deref(),
                    &task_path_scopes,
                )
                .await
            }
            "command_run" => {
                self.dispatch_command_run(
                    &context,
                    arguments,
                    project_folder.as_deref(),
                    &task_path_scopes,
                )
                .await
            }
            "shell_write" => {
                let input: ShellWrite = parse(arguments)?;
                self.dispatch_shell_write(&context, input).await
            }
            "shell_wait" => {
                let input: ShellWait = parse(arguments)?;
                self.dispatch_shell_wait(&context, input).await
            }
            "shell_read" => {
                let input: ShellRead = parse(arguments)?;
                let (result, usage) = self
                    .shell
                    .read_with_context(
                        &context,
                        &input.session_id,
                        input.after_sequence,
                        input.max_events.clamp(1, 2_000),
                    )
                    .await?;
                self.persist_shell_events(&context, &result).await?;
                value_with_usage(result, usage)
            }
            "shell_signal" => {
                let input: ShellSignalInput = parse(arguments)?;
                self.dispatch_shell_signal(&context, input).await
            }
            "shell_resize" => {
                let input: ShellResize = parse(arguments)?;
                self.dispatch_shell_resize(&context, input).await
            }
            "shell_close" => {
                let input: ShellClose = parse(arguments)?;
                self.dispatch_shell_close(&context, input).await
            }
            "shell_list" => self.dispatch_shell_list().await,
            "shell_inspect" => {
                let input: SessionInput = parse(arguments)?;
                self.dispatch_shell_inspect(input).await
            }
            "workspace_roots" => {
                if !all_project_folders.is_empty() {
                    value(all_project_folders)
                } else {
                    value(task_path_scopes)
                }
            }
            "project_context" => {
                let input: ProjectContextInput = parse(arguments)?;
                let folder = project_folder.ok_or_else(|| {
                    RuntimeError::new(
                        "project_folder_required",
                        "project context requires the current task project folder",
                    )
                })?;
                value(
                    chatcmd_runtime::ProjectContextService::default()
                        .load_with_options(&folder, &input.target_paths, input.policy, input.range)
                        .await?,
                )
            }
            "blob_begin" => value(self.blob_store.begin(
                &context,
                parse::<chatcmd_runtime::BlobBeginRequest>(arguments)?,
            )?),
            "blob_write_chunk" => value(self.blob_store.write_chunk(
                &context,
                parse::<chatcmd_runtime::BlobChunkRequest>(arguments)?,
            )?),
            "blob_status" => {
                let input: BlobStatusInput = parse(arguments)?;
                value(self.blob_store.status_with_budget(
                    &context,
                    &input.upload_id,
                    &input.budget,
                )?)
            }
            "blob_seal" => value(self.blob_store.seal(
                &context,
                parse::<chatcmd_runtime::BlobSealRequest>(arguments)?,
            )?),
            "blob_abort" => {
                let input: BlobStatusInput = parse(arguments)?;
                value(self.blob_store.abort_with_budget(
                    &context,
                    &input.upload_id,
                    &input.budget,
                )?)
            }
            "git_status" => {
                let input: GitCwdInput = parse(arguments)?;
                let cwd = self.resolve_git_cwd(&context, input.cwd).await?;
                let output = git
                    .status_with_options(&cwd, &input.options, context.cancellation.clone())
                    .await?;
                value(self.register_git_artifact(&context, output).await?)
            }
            "git_diff" => {
                let input: GitDiff = parse(arguments)?;
                let cwd = self.resolve_git_cwd(&context, input.cwd).await?;
                let output = git
                    .diff_with_options(
                        &cwd,
                        input.staged,
                        input.stat,
                        input.path.as_deref(),
                        &input.options,
                        context.cancellation.clone(),
                    )
                    .await?;
                value(self.register_git_artifact(&context, output).await?)
            }
            "git_log" => {
                let input: GitLog = parse(arguments)?;
                let cwd = self.resolve_git_cwd(&context, input.cwd).await?;
                let output = git
                    .log_with_options(
                        &cwd,
                        input.count,
                        input.path.as_deref(),
                        &input.options,
                        context.cancellation.clone(),
                    )
                    .await?;
                value(self.register_git_artifact(&context, output).await?)
            }
            "git_branch" => {
                let input: GitCwdInput = parse(arguments)?;
                let cwd = self.resolve_git_cwd(&context, input.cwd).await?;
                let output = git
                    .branch_with_options(&cwd, &input.options, context.cancellation.clone())
                    .await?;
                value(self.register_git_artifact(&context, output).await?)
            }
            "git_show" => {
                let input: GitShow = parse(arguments)?;
                let cwd = self.resolve_git_cwd(&context, input.cwd).await?;
                let output = git
                    .show_with_options(
                        &cwd,
                        &input.revision,
                        input.path.as_deref(),
                        &input.options,
                        context.cancellation.clone(),
                    )
                    .await?;
                value(self.register_git_artifact(&context, output).await?)
            }
            "git_commit" => {
                let input: GitCommit = parse(arguments)?;
                let cwd = self.resolve_git_cwd(&context, input.cwd).await?;
                if input.preview_only {
                    return value(
                        git.preview_commit_with_options(
                            &cwd,
                            input.all,
                            &input.paths,
                            &input.options,
                            context.cancellation.clone(),
                        )
                        .await?,
                    );
                }
                let output = if let Some(preview) = input.expected_preview.as_ref() {
                    git.commit_previewed_with_options(
                        &cwd,
                        &input.message,
                        input.all,
                        &input.paths,
                        preview,
                        &input.options,
                        context.cancellation.clone(),
                    )
                    .await?
                } else {
                    git.commit_with_options(
                        &cwd,
                        &input.message,
                        input.all,
                        &input.paths,
                        &input.options,
                        context.cancellation.clone(),
                    )
                    .await?
                };
                value(self.register_git_artifact(&context, output).await?)
            }
            "process_list" => value(self.process.list().await?),
            "process_inspect" => {
                let input: ProcessInput = parse(arguments)?;
                value(self.process.inspect(input.process_id).await?)
            }
            "process_kill" => {
                let input: ProcessKill = parse(arguments)?;
                self.process
                    .kill(&context, input.process_id, input.entire_tree)
                    .await?;
                Ok(json!({ "killed": true }))
            }
            "skills_list" => value(
                self.skills
                    .list_for_task(context.task_id.as_deref(), project_folder.as_deref())
                    .await?,
            ),
            "skill_read" => {
                let input: SkillInput = parse(arguments)?;
                value(
                    self.skills
                        .read_for_task(
                            context.task_id.as_deref(),
                            &input.skill_id,
                            input.tier.as_deref(),
                            input.tool.as_deref(),
                            project_folder.as_deref(),
                        )
                        .await?,
                )
            }
            "task_get" => {
                let id = context_task_id(&context)?;
                Ok(self
                    .repository
                    .task(&id)
                    .await
                    .map_err(storage_error)?
                    .map_or(Value::Null, task_json))
            }
            "task_list" => Ok(Value::Array(
                self.repository
                    .list_tasks(200)
                    .await
                    .map_err(storage_error)?
                    .into_iter()
                    .map(task_json)
                    .collect(),
            )),
            "task_set_execution_mode" => {
                let input: ExecutionModeInput = parse(arguments)?;
                let id = context_task_id(&context)?;
                let mode = match input.mode.as_str() {
                    "approval" => ExecutionMode::Approval,
                    "allow" | "safe" | "unrestricted" => ExecutionMode::Allow,
                    "deny" => ExecutionMode::Deny,
                    _ => return Err(invalid("mode", "must be approval, allow, or deny")),
                };
                self.repository
                    .set_execution_mode(&TaskExecutionMode {
                        task_id: id,
                        mode,
                        updated_at_ms: now_ms(),
                    })
                    .await
                    .map_err(storage_error)?;
                Ok(json!({ "mode": mode.as_str() }))
            }
            "task_artifact_list" => self.list_artifacts(&context).await,
            "task_artifact_create" => self.create_artifact(&context, arguments).await,
            "task_artifact_read" => self.read_artifact(&context, arguments).await,
            "agent_user_message" => {
                let input: UserMessageInput = parse(arguments)?;
                self.save_user_message(&context, &input.content).await
            }
            "agent_progress" => {
                let input: ProgressInput = parse(arguments)?;
                self.save_agent_event(
                    &context,
                    "progress",
                    &input.message,
                    input.suggested_title.as_deref(),
                )
                .await
            }
            "agent_plan_question" => {
                let input: PlanQuestionInput = parse(arguments)?;
                self.ask_plan_question_with_kind(
                    &context,
                    input.question,
                    input.options,
                    input.question_kind,
                )
                .await
            }
            "agent_subagent_start" => {
                let input: SubagentStartInput = parse(arguments)?;
                super::subagent_contract::validate_delegation_contract(&input)?;
                self.register_subagent(
                    &context,
                    &input.name,
                    &input.request,
                    input.approval_grant.as_ref(),
                )
                .await
            }
            "agent_subagent_wait" => {
                let input: SubagentWaitInput = parse(arguments)?;
                self.wait_for_subagent_reports(&context, &input).await
            }
            "agent_turn_complete" => self.complete_agent_turn(&context, arguments).await,
            _ => Err(RuntimeError::new("tool_not_found", "unknown MCP tool")),
        }
    }
}

use helpers::*;
