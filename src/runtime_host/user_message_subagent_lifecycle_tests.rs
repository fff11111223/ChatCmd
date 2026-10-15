use super::*;

#[tokio::test]
async fn model_may_start_subagent_for_ordinary_turn_and_capacity_is_enforced() {
    let (host, agent_id, _directory) = test_host().await;
    sqlx::query(
        "INSERT INTO settings(key,value_json,updated_at_ms) VALUES('ui_subagentConcurrency','1',0)",
    )
    .execute(host.repository.pool())
    .await
    .expect("enable one child slot");
    let scope = "conversation-subagent-autonomous-delegation";
    let turn = "turn-subagent-autonomous-delegation";
    let parent = host
        .call_persisted(
            "agent_user_message",
            turn_context(
                "parent-user-autonomous-delegation",
                &agent_id,
                "agent_user_message",
                turn,
                scope,
            ),
            json!({"content":"Rà soát toàn bộ source code và sub agent để tìm lỗi"}),
        )
        .await
        .expect("sync ordinary parent turn");
    assert_eq!(parent["subagentPolicy"]["explicitUserIntent"], false);
    assert_eq!(parent["subagentPolicy"]["enabled"], true);
    assert_eq!(parent["subagentPolicy"]["maxConcurrent"], 1);
    assert_eq!(parent["subagentPolicy"]["maxPerRootTurn"], 20);
    assert!(
        parent["subagentPolicy"]["instruction"]
            .as_str()
            .unwrap()
            .contains("does not prohibit it")
    );
    let parent_task = parent["taskId"].as_str().expect("parent task");
    let start = |request_id: &str| {
        let mut context = OperationContext::new(request_id, &agent_id, "agent_subagent_start");
        context.task_id = Some(parent_task.to_owned());
        context.turn_id = Some(turn.to_owned());
        context
    };
    let first = host
        .call_persisted(
            "agent_subagent_start",
            start("autonomous-child-start"),
            json!({"name":"Frontend reviewer","request":"Inspect the frontend independently"}),
        )
        .await
        .expect("ordinary user intent does not block useful delegation");
    assert_eq!(first["duplicate"], false);
    let error = host
        .call_persisted(
            "agent_subagent_start",
            start("parallel-limit-child-start"),
            json!({"name":"Backend reviewer","request":"Inspect the backend independently"}),
        )
        .await
        .expect_err("an extra child must be rejected when the slot limit is reached");
    assert_eq!(error.code, "subagent_capacity_exhausted");
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM subagent_runs WHERE parent_task_id=?")
            .bind(parent_task)
            .fetch_one(host.repository.pool())
            .await
            .expect("count unexpected children");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn disabled_subagents_are_rejected_by_backend_policy() {
    let (host, agent_id, _directory) = test_host().await;
    sqlx::query("INSERT INTO settings(key,value_json,updated_at_ms) VALUES('ui_subagentConcurrency','0',0) ON CONFLICT(key) DO UPDATE SET value_json='0',updated_at_ms=0")
        .execute(host.repository.pool()).await.expect("disable subagents");
    let scope = "conversation-subagent-disabled";
    let turn = "turn-subagent-disabled";
    let parent = host
        .call_persisted(
            "agent_user_message",
            turn_context(
                "parent-user-subagent-disabled",
                &agent_id,
                "agent_user_message",
                turn,
                scope,
            ),
            json!({"content":"Check this simple issue"}),
        )
        .await
        .expect("sync parent turn");
    assert_eq!(parent["subagentPolicy"]["enabled"], false);
    let mut start =
        OperationContext::new("disabled-subagent-start", &agent_id, "agent_subagent_start");
    start.task_id = Some(parent["taskId"].as_str().unwrap().to_owned());
    start.turn_id = Some(turn.to_owned());
    let error = host
        .call_persisted(
            "agent_subagent_start",
            start,
            json!({"name":"Disabled child","request":"Inspect one file"}),
        )
        .await
        .expect_err("disabled subagents remain unavailable");
    assert_eq!(error.code, "subagents_disabled");
}

#[tokio::test]
async fn explicit_parallel_request_can_start_multiple_children() {
    let (host, agent_id, _directory) = test_host().await;
    sqlx::query("INSERT INTO settings(key,value_json,updated_at_ms) VALUES('ui_subagentConcurrency','2',0) ON CONFLICT(key) DO UPDATE SET value_json='2',updated_at_ms=0")
        .execute(host.repository.pool()).await.expect("enable two child slots");
    let scope = "conversation-explicit-parallel-subagents";
    let turn = "turn-explicit-parallel-subagents";
    let parent = host
        .call_persisted(
            "agent_user_message",
            turn_context(
                "parent-explicit-parallel",
                &agent_id,
                "agent_user_message",
                turn,
                scope,
            ),
            json!({"content":"分別檢查前端、後端及工具呼叫流程"}),
        )
        .await
        .expect("sync explicitly partitioned parent turn");
    assert_eq!(parent["subagentPolicy"]["explicitUserIntent"], true);
    let parent_task = parent["taskId"].as_str().expect("parent task");
    for (request_id, name, request) in [
        (
            "explicit-frontend-child",
            "Frontend reviewer",
            "Inspect frontend",
        ),
        (
            "explicit-backend-child",
            "Backend reviewer",
            "Inspect backend and tool flow",
        ),
    ] {
        let mut context = OperationContext::new(request_id, &agent_id, "agent_subagent_start");
        context.task_id = Some(parent_task.to_owned());
        context.turn_id = Some(turn.to_owned());
        host.call_persisted(
            "agent_subagent_start",
            context,
            json!({"name":name,"request":request}),
        )
        .await
        .expect("explicitly requested parallel child should register");
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM subagent_runs WHERE parent_task_id=? AND parent_turn_id=?",
    )
    .bind(parent_task)
    .bind(turn)
    .fetch_one(host.repository.pool())
    .await
    .expect("count children");
    assert_eq!(count, 2);
}
