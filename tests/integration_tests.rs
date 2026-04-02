use futures::StreamExt;
use hatchet_sdk::{HatchetError, Runnable};

mod common;
use common::{SimpleInput, SimpleOutput, TestHarness};

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
