use futures::StreamExt;
use hatchet_sdk::{HatchetError, Runnable};

mod common;
use common::{SimpleInput, SimpleOutput, TestHarness, hatchet_version_at_least};

#[tokio::test]
async fn test_run_returns_job_output() {
    let t = TestHarness::new("run-output").await;

    let task = t
        .hatchet
        .task(
            &t.prefixed("step"),
            async move |input: SimpleInput,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
                Ok(SimpleOutput {
                    transformed_message: input.message.to_lowercase(),
                })
            },
        )
        .build()
        .unwrap();

    let _worker = t.spawn_worker_for_task(&task).await;

    let options = hatchet_sdk::TriggerWorkflowOptionsBuilder::default()
        .additional_metadata(Some(serde_json::json!({
            "environment": "dev",
        })))
        .build()
        .unwrap();

    assert_eq!(
        "uppercase",
        task.run(
            &SimpleInput {
                message: "UPPERCASE".to_string()
            },
            Some(&options)
        )
        .await
        .unwrap()
        .transformed_message
    );
}

#[tokio::test]
async fn test_run_returns_error_if_job_fails() {
    let t = TestHarness::new("run-error").await;

    let task = t
        .hatchet
        .task(
            &t.prefixed("step"),
            async move |_input: SimpleInput,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
                anyhow::bail!("Test failed.")
            },
        )
        .build()
        .unwrap();

    let _worker = t.spawn_worker_for_task(&task).await;

    let output = task
        .run(
            &SimpleInput {
                message: "UPPERCASE".to_string(),
            },
            None,
        )
        .await;

    assert!(matches!(output, Err(HatchetError::WorkflowFailed(_))));
}

#[tokio::test]
async fn test_dynamically_spawn_child_workflow() {
    let t = TestHarness::new("dynamic-child").await;

    let child_task = t
        .hatchet
        .task(
            &t.prefixed("child"),
            async move |_input: hatchet_sdk::EmptyModel,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({"output": "Hello from child task"}))
            },
        )
        .build()
        .unwrap();

    let child_task_clone = child_task.clone();

    let parent_task = t
        .hatchet
        .task(
            &t.prefixed("parent"),
            async move |_input: hatchet_sdk::EmptyModel,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<serde_json::Value> {
                Ok(child_task
                    .run(&hatchet_sdk::EmptyModel, None)
                    .await
                    .unwrap())
            },
        )
        .build()
        .unwrap();

    let _worker = t
        .spawn_worker_for_tasks(&[&parent_task, &child_task_clone])
        .await;

    let output = parent_task
        .run(&hatchet_sdk::EmptyModel, None)
        .await
        .unwrap();

    assert_eq!("Hello from child task", output.get("output").unwrap());
}

#[tokio::test]
async fn test_dag_workflow() {
    let t = TestHarness::new("dag").await;

    let parent_task = t
        .hatchet
        .task(
            &t.prefixed("parent"),
            async move |_input: hatchet_sdk::EmptyModel,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({"message": "I am your father"}))
            },
        )
        .build()
        .unwrap();

    let parent_step_name = t.prefixed("parent");
    let child_task = t
        .hatchet
        .task(
            &t.prefixed("child"),
            async move |_input: hatchet_sdk::EmptyModel,
                        ctx: hatchet_sdk::Context|
                        -> anyhow::Result<serde_json::Value> {
                let parent_output = ctx.parent_output(&parent_step_name).await?;
                let message = parent_output.get("message").unwrap();
                Ok(serde_json::json!({"output": format!("Parent said: {}", message.to_string())}))
            },
        )
        .build()
        .unwrap()
        .add_parent(&parent_task);

    let dag_workflow = t
        .hatchet
        .workflow::<hatchet_sdk::EmptyModel, serde_json::Value>(&t.prefixed("workflow"))
        .build()
        .unwrap()
        .add_task(&parent_task)
        .add_task(&child_task);

    let _worker = t.spawn_worker_for_workflow(&dag_workflow).await;

    let output = dag_workflow.run(&hatchet_sdk::EmptyModel, None).await;

    let child_name = t.prefixed("child");
    assert_eq!(
        "Parent said: \"I am your father\"",
        output
            .unwrap()
            .get(&child_name)
            .unwrap()
            .get("output")
            .unwrap()
    );
}

#[tokio::test]
async fn test_streaming() {
    let t = TestHarness::new("streaming").await;

    let expected_chunks: Vec<String> = (0..5).map(|i| format!("chunk-{}", i)).collect();
    let chunks_to_send = expected_chunks.clone();

    let task = t
        .hatchet
        .task(
            &t.prefixed("stream-step"),
            async move |_input: SimpleInput,
                        ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                for chunk in &chunks_to_send {
                    ctx.put_stream(chunk.as_bytes().to_vec()).await?;
                }
                Ok(SimpleOutput {
                    transformed_message: "done".to_string(),
                })
            },
        )
        .build()
        .unwrap();

    let _worker = t.spawn_worker_for_task(&task).await;

    let run_id = task
        .run_no_wait(
            &SimpleInput {
                message: "test".to_string(),
            },
            None,
        )
        .await
        .unwrap();

    let mut hatchet_consumer = t.hatchet.clone();
    let mut stream = hatchet_consumer
        .workflow_rest_client
        .subscribe_to_stream(&run_id)
        .await
        .unwrap();

    let mut received_chunks: Vec<String> = Vec::new();
    let timeout = tokio::time::Duration::from_secs(30);
    let start = tokio::time::Instant::now();

    while let Ok(Some(chunk)) =
        tokio::time::timeout(timeout.saturating_sub(start.elapsed()), stream.next()).await
    {
        match chunk {
            Ok(data) => {
                received_chunks.push(String::from_utf8(data).unwrap());
                if received_chunks.len() == expected_chunks.len() {
                    break;
                }
            }
            Err(e) => panic!("Stream error: {}", e),
        }
    }

    assert_eq!(expected_chunks, received_chunks);
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct AddInput {
    first: i64,
    second: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AddOutput {
    value: i64,
}

#[tokio::test]
async fn test_workflow_with_input_json_schema() {
    let t = TestHarness::new("json-schema").await;

    let schema = schemars::schema_for!(AddInput);
    let schema_value = serde_json::to_value(schema).unwrap();

    let task = t
        .hatchet
        .task(
            &t.prefixed("add"),
            async move |input: AddInput,
                        _context: hatchet_sdk::Context|
                        -> anyhow::Result<AddOutput> {
                Ok(AddOutput {
                    value: input.first + input.second,
                })
            },
        )
        .input_json_schema(Some(schema_value))
        .build()
        .unwrap();

    let _worker = t.spawn_worker_for_task(&task).await;

    let output = task
        .run(
            &AddInput {
                first: 3,
                second: 7,
            },
            None,
        )
        .await
        .unwrap();

    assert_eq!(10, output.value);
}

#[tokio::test]
async fn test_cron_lifecycle() {
    let t = TestHarness::new("cron-lifecycle").await;
    let task = t.simple_task("task");
    let _worker = t.spawn_worker_for_task(&task).await;

    let task_name = t.prefixed("task");
    let cron = t
        .hatchet
        .crons
        .create(
            &task_name,
            hatchet_sdk::CreateCronOpts {
                name: t.prefixed("hourly"),
                expression: "0 * * * *".to_string(),
                input: serde_json::json!({"message": "cron-input"}),
                additional_metadata: Some(serde_json::json!({"env": "test"})),
                priority: Some(2),
            },
        )
        .await
        .unwrap();

    assert!(cron.enabled);
    assert_eq!(cron.priority, Some(2));

    let fetched = t.hatchet.crons.get(&cron.metadata_id).await.unwrap();
    assert_eq!(fetched.metadata_id, cron.metadata_id);

    let list = t.hatchet.crons.list(Default::default()).await.unwrap();
    assert!(list.rows.iter().any(|r| r.metadata_id == cron.metadata_id));

    t.hatchet.crons.delete(&cron.metadata_id).await.unwrap();

    let after_delete = t.hatchet.crons.get(&cron.metadata_id).await;
    assert!(after_delete.is_err());

    let list_after = t.hatchet.crons.list(Default::default()).await.unwrap();
    assert!(
        !list_after
            .rows
            .iter()
            .any(|r| r.metadata_id == cron.metadata_id)
    );
}

#[tokio::test]
async fn test_schedule_lifecycle() {
    let t = TestHarness::new("schedule-lifecycle").await;
    let task = t.simple_task("task");
    let _worker = t.spawn_worker_for_task(&task).await;

    let task_name = t.prefixed("task");
    let trigger_at = hatchet_sdk::chrono::Utc::now() + hatchet_sdk::chrono::Duration::hours(1);
    let scheduled = t
        .hatchet
        .schedules
        .create(
            &task_name,
            hatchet_sdk::CreateScheduleOpts {
                trigger_at,
                input: serde_json::json!({"message": "scheduled-input"}),
                additional_metadata: Some(serde_json::json!({"env": "test"})),
                priority: Some(3),
            },
        )
        .await
        .unwrap();

    assert!(!scheduled.metadata_id.is_empty());
    assert_eq!(scheduled.priority, Some(3));

    // schedules.get() returns 500 on Hatchet < v0.75
    if hatchet_version_at_least(0, 75) {
        let fetched = t
            .hatchet
            .schedules
            .get(&scheduled.metadata_id)
            .await
            .unwrap();
        assert_eq!(fetched.metadata_id, scheduled.metadata_id);
    }

    let list = t.hatchet.schedules.list(Default::default()).await.unwrap();
    assert!(
        list.rows
            .iter()
            .any(|r| r.metadata_id == scheduled.metadata_id)
    );

    t.hatchet
        .schedules
        .delete(&scheduled.metadata_id)
        .await
        .unwrap();

    if hatchet_version_at_least(0, 75) {
        let after_delete = t.hatchet.schedules.get(&scheduled.metadata_id).await;
        assert!(after_delete.is_err());
    }

    let list_after = t.hatchet.schedules.list(Default::default()).await.unwrap();
    assert!(
        !list_after
            .rows
            .iter()
            .any(|r| r.metadata_id == scheduled.metadata_id)
    );
}

#[tokio::test]
async fn test_task_cron_convenience() {
    let t = TestHarness::new("cron-conv").await;
    let task = t.simple_task("task");
    let _worker = t.spawn_worker_for_task(&task).await;

    let cron = task
        .cron(
            &t.prefixed("every5"),
            "*/5 * * * *",
            &SimpleInput {
                message: "via-convenience".to_string(),
            },
            Some(&hatchet_sdk::CronOptions {
                additional_metadata: Some(serde_json::json!({"source": "convenience"})),
                priority: Some(1),
            }),
        )
        .await
        .unwrap();

    assert_eq!(cron.priority, Some(1));

    let list = t.hatchet.crons.list(Default::default()).await.unwrap();
    assert!(list.rows.iter().any(|r| r.metadata_id == cron.metadata_id));

    t.hatchet.crons.delete(&cron.metadata_id).await.unwrap();
}

#[tokio::test]
async fn test_task_schedule_convenience() {
    let t = TestHarness::new("sched-conv").await;
    let task = t.simple_task("task");
    let _worker = t.spawn_worker_for_task(&task).await;

    let trigger_at = hatchet_sdk::chrono::Utc::now() + hatchet_sdk::chrono::Duration::hours(2);
    let scheduled = task
        .schedule(
            trigger_at,
            &SimpleInput {
                message: "via-convenience".to_string(),
            },
            Some(&hatchet_sdk::ScheduleOptions {
                additional_metadata: Some(serde_json::json!({"source": "convenience"})),
                priority: Some(2),
            }),
        )
        .await
        .unwrap();

    assert!(!scheduled.metadata_id.is_empty());
    assert_eq!(scheduled.priority, Some(2));

    let list = t.hatchet.schedules.list(Default::default()).await.unwrap();
    assert!(
        list.rows
            .iter()
            .any(|r| r.metadata_id == scheduled.metadata_id)
    );

    t.hatchet
        .schedules
        .delete(&scheduled.metadata_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_workflow_cron_convenience() {
    let t = TestHarness::new("wf-cron-conv").await;

    let task = t.simple_task("step");
    let workflow = t
        .hatchet
        .workflow::<SimpleInput, SimpleOutput>(&t.prefixed("wf"))
        .build()
        .unwrap()
        .add_task(&task);

    let _worker = t.spawn_worker_for_workflow(&workflow).await;

    let cron = workflow
        .cron(
            &t.prefixed("wf-cron"),
            "0 * * * *",
            &SimpleInput {
                message: "workflow-cron".to_string(),
            },
            None,
        )
        .await
        .unwrap();

    let list = t.hatchet.crons.list(Default::default()).await.unwrap();
    assert!(list.rows.iter().any(|r| r.metadata_id == cron.metadata_id));

    t.hatchet.crons.delete(&cron.metadata_id).await.unwrap();
}

#[tokio::test]
async fn test_workflow_schedule_convenience() {
    let t = TestHarness::new("wf-sched-conv").await;

    let task = t.simple_task("step");
    let workflow = t
        .hatchet
        .workflow::<SimpleInput, SimpleOutput>(&t.prefixed("wf"))
        .build()
        .unwrap()
        .add_task(&task);

    let _worker = t.spawn_worker_for_workflow(&workflow).await;

    let trigger_at = hatchet_sdk::chrono::Utc::now() + hatchet_sdk::chrono::Duration::hours(3);
    let scheduled = workflow
        .schedule(
            trigger_at,
            &SimpleInput {
                message: "workflow-schedule".to_string(),
            },
            None,
        )
        .await
        .unwrap();

    assert!(!scheduled.metadata_id.is_empty());

    let list = t.hatchet.schedules.list(Default::default()).await.unwrap();
    assert!(
        list.rows
            .iter()
            .any(|r| r.metadata_id == scheduled.metadata_id)
    );

    t.hatchet
        .schedules
        .delete(&scheduled.metadata_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_task_with_flow_control() {
    let t = TestHarness::new("flow-control").await;

    let task = t
        .hatchet
        .task(
            &t.prefixed("step"),
            async move |input: SimpleInput,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
                Ok(SimpleOutput {
                    transformed_message: format!("synced:{}", input.message),
                })
            },
        )
        .concurrency(vec![hatchet_sdk::ConcurrencyExpression {
            expression: "\"test\"".to_string(),
            max_runs: 2,
            limit_strategy: hatchet_sdk::ConcurrencyLimitStrategy::GroupRoundRobin,
        }])
        .rate_limits(vec![hatchet_sdk::RateLimit::Dynamic {
            key: "test-limit".to_string(),
            key_expr: "\"test\"".to_string(),
            units: 1,
            limit: 10,
            duration: hatchet_sdk::RateLimitDuration::Minute,
        }])
        .build()
        .unwrap();

    let _worker = t.spawn_worker_for_task(&task).await;

    let output = task
        .run(
            &SimpleInput {
                message: "payload".to_string(),
            },
            None,
        )
        .await
        .unwrap();

    assert_eq!("synced:payload", output.transformed_message);
}

#[tokio::test]
async fn test_concurrency_cancel_newest() {
    let t = TestHarness::new("conc-cancel").await;

    let task = t
        .hatchet
        .task(
            &t.prefixed("step"),
            async move |_input: SimpleInput,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                Ok(SimpleOutput {
                    transformed_message: "done".to_string(),
                })
            },
        )
        .concurrency(vec![hatchet_sdk::ConcurrencyExpression {
            expression: "\"test\"".to_string(),
            max_runs: 1,
            limit_strategy: hatchet_sdk::ConcurrencyLimitStrategy::CancelNewest,
        }])
        .build()
        .unwrap();

    let _worker = t.spawn_worker_for_task(&task).await;

    // Run first task
    let _run1 = task
        .run_no_wait(
            &SimpleInput {
                message: "payload".to_string(),
            },
            None,
        )
        .await
        .unwrap();

    // Wait for run1 to transition to Running
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Run second task, which should exceed max_runs and be cancelled
    let run2 = task
        .run_no_wait(
            &SimpleInput {
                message: "payload".to_string(),
            },
            None,
        )
        .await
        .unwrap();

    // Poll until run2 reaches a terminal state (Cancelled or Failed).
    // The concurrency engine processes cancellations asynchronously.
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(15);
    let mut status2 = String::new();
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let res2 = t
            .hatchet
            .workflow_rest_client
            .get(&run2)
            .await
            .unwrap();
        status2 = format!("{:?}", res2.run.status);
        println!("STATUS2: {}, RUN2 RESULT: {:?}", status2, res2);
        if status2 == "Cancelled" || status2 == "Failed" {
            break;
        }
    }
    assert!(
        status2 == "Cancelled" || status2 == "Failed",
        "Status was {}",
        status2
    );
}

/// Verifies that concurrency expressions defined on a Task are correctly
/// hoisted to the Workflow when the task is added via `Workflow::add_task()`.
///
/// This mirrors `test_concurrency_cancel_newest` but wraps the task in a
/// Workflow. Before the hoisting fix, concurrency would have been silently
/// dropped and the second run would NOT be cancelled.
#[tokio::test]
async fn test_workflow_hoists_task_concurrency() {
    let t = TestHarness::new("wf-conc").await;

    // Define a task with CancelNewest concurrency (max_runs: 1).
    let task = t
        .hatchet
        .task(
            &t.prefixed("step"),
            async move |_input: SimpleInput,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                Ok(SimpleOutput {
                    transformed_message: "done".to_string(),
                })
            },
        )
        .concurrency(vec![hatchet_sdk::ConcurrencyExpression {
            expression: "\"test\"".to_string(),
            max_runs: 1,
            limit_strategy: hatchet_sdk::ConcurrencyLimitStrategy::CancelNewest,
        }])
        .build()
        .unwrap();

    // Wrap the task in a Workflow. The concurrency should be hoisted.
    let workflow = t
        .hatchet
        .workflow::<SimpleInput, serde_json::Value>(&t.prefixed("workflow"))
        .build()
        .unwrap()
        .add_task(&task);

    let _worker = t.spawn_worker_for_workflow(&workflow).await;

    // Run first workflow — should start executing
    let _run1 = workflow
        .run_no_wait(
            &SimpleInput {
                message: "payload".to_string(),
            },
            None,
        )
        .await
        .unwrap();

    // Wait for run1 to transition to Running
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Run second workflow — should exceed max_runs and be cancelled
    let run2 = workflow
        .run_no_wait(
            &SimpleInput {
                message: "payload".to_string(),
            },
            None,
        )
        .await
        .unwrap();

    // Poll until run2 reaches a terminal state (Cancelled or Failed).
    // The concurrency engine processes cancellations asynchronously.
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(15);
    let mut status2 = String::new();
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let res2 = t
            .hatchet
            .workflow_rest_client
            .get(&run2)
            .await
            .unwrap();
        status2 = format!("{:?}", res2.run.status);
        println!("STATUS2: {}, RUN2 RESULT: {:?}", status2, res2);
        if status2 == "Cancelled" || status2 == "Failed" {
            break;
        }
    }
    assert!(
        status2 == "Cancelled" || status2 == "Failed",
        "Status was {}",
        status2
    );
}
