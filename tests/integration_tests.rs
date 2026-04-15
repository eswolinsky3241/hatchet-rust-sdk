use futures::StreamExt;
use hatchet_sdk::worker::worker::WorkerBuilder;
use hatchet_sdk::{Hatchet, HatchetError, Register, Runnable};
use serde::{Deserialize, Serialize};

mod common;
use common::{SimpleInput, SimpleOutput};

#[tokio::test]
async fn test_run_returns_job_output() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    let task = hatchet
        .task(
            "step1",
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

    let task_clone = task.clone();
    let worker_handle: tokio::task::JoinHandle<()> = tokio::spawn(async move {
        WorkerBuilder::default()
            .name(String::from("test-worker"))
            .client(hatchet.clone())
            .build()
            .unwrap()
            .add_task_or_workflow(&task_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

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
    worker_handle.abort()
}

#[tokio::test]
async fn test_run_returns_error_if_job_fails() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    let task = hatchet
        .task(
            "step1",
            async move |_input: SimpleInput,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
                anyhow::bail!("Test failed.")
            },
        )
        .build()
        .unwrap();

    let task_clone = task.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet
            .worker("test-worker")
            .build()
            .unwrap()
            .add_task_or_workflow(&task_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let output = task
        .run(
            &SimpleInput {
                message: "UPPERCASE".to_string(),
            },
            None,
        )
        .await;

    let _err = HatchetError::WorkflowFailed("Task execution failed: Test failed".to_string());

    assert!(matches!(output, Err(_err)));
    worker_handle.abort()
}

#[tokio::test]
async fn test_dynamically_spawn_child_workflow() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    let child_task = hatchet
        .task(
            "child_task",
            async move |_input: hatchet_sdk::EmptyModel,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({"output": "Hello from child task"}))
            },
        )
        .build()
        .unwrap();

    let child_task_clone = child_task.clone();

    let parent_task = hatchet
        .task(
            "parent_task",
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

    let task_clone = parent_task.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet_sdk::worker::worker::WorkerBuilder::default()
            .name(String::from("test-worker"))
            .client(hatchet.clone())
            .build()
            .unwrap()
            .add_task_or_workflow(&task_clone)
            .add_task_or_workflow(&child_task_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let output = parent_task
        .run(&hatchet_sdk::EmptyModel, None)
        .await
        .unwrap();

    assert_eq!("Hello from child task", output.get("output").unwrap());
    worker_handle.abort()
}

#[tokio::test]
async fn test_dag_workflow() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    let parent_task = hatchet
        .task(
            "parent_task",
            async move |_input: hatchet_sdk::EmptyModel,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({"message": "I am your father"}))
            },
        )
        .build()
        .unwrap();

    let child_task = hatchet
        .task(
            "child_task",
            async move |_input: hatchet_sdk::EmptyModel,
                        ctx: hatchet_sdk::Context|
                        -> anyhow::Result<serde_json::Value> {
                let parent_output = ctx.parent_output("parent_task").await?;
                let message = parent_output.get("message").unwrap();
                Ok(serde_json::json!({"output": format!("Parent said: {}", message.to_string())}))
            },
        )
        .build()
        .unwrap()
        .add_parent(&parent_task);

    let dag_workflow = hatchet
        .workflow::<hatchet_sdk::EmptyModel, serde_json::Value>("parent-workflow")
        .build()
        .unwrap()
        .add_task(&parent_task)
        .add_task(&child_task);

    let dag_workflow_clone = dag_workflow.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet_sdk::worker::worker::WorkerBuilder::default()
            .name(String::from("test-worker"))
            .client(hatchet.clone())
            .build()
            .unwrap()
            .add_task_or_workflow(&dag_workflow_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let output = dag_workflow.run(&hatchet_sdk::EmptyModel, None).await;

    assert_eq!(
        "Parent said: \"I am your father\"",
        output
            .unwrap()
            .get("child_task")
            .unwrap()
            .get("output")
            .unwrap()
    );
    worker_handle.abort()
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct AddInput {
    first: i64,
    second: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AddOutput {
    value: i64,
}

#[tokio::test]
async fn test_streaming() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    let expected_chunks: Vec<String> = (0..5).map(|i| format!("chunk-{}", i)).collect();
    let chunks_to_send = expected_chunks.clone();

    let task = hatchet
        .task(
            "stream-step",
            async move |_input: SimpleInput,
                        ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
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

    let task_clone = task.clone();
    let mut hatchet_consumer = hatchet.clone();
    let worker_handle = tokio::spawn(async move {
        WorkerBuilder::default()
            .name(String::from("test-stream-worker"))
            .client(hatchet.clone())
            .build()
            .unwrap()
            .add_task_or_workflow(&task_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Trigger the task — run_no_wait returns the run ID immediately before the task
    // is scheduled, so subscribing right after ensures we don't miss any stream events.
    let run_id = task
        .run_no_wait(
            &SimpleInput {
                message: "test".to_string(),
            },
            None,
        )
        .await
        .unwrap();

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
    worker_handle.abort();
}

// Verifies that workflows with input_json_schema can be registered and executed.
#[tokio::test]
async fn test_workflow_with_input_json_schema() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    let schema = schemars::schema_for!(AddInput);
    let schema_value = serde_json::to_value(schema).unwrap();

    let task = hatchet
        .task(
            "add",
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

    let task_clone: hatchet_sdk::Task<AddInput, AddOutput> = task.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet
            .worker("test-worker")
            .build()
            .unwrap()
            .add_task_or_workflow(&task_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register workflow.
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

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
    worker_handle.abort()
}

#[tokio::test]
async fn test_task_with_flow_control() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    let task = hatchet
        .task(
            "flow-control-step",
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

    let task_clone = task.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet_sdk::worker::worker::WorkerBuilder::default()
            .name(String::from("flow-control-worker"))
            .client(hatchet.clone())
            .build()
            .unwrap()
            .add_task_or_workflow(&task_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let output = task
        .run(
            &SimpleInput {
                message: "payload".to_string()
            },
            None,
        )
        .await
        .unwrap();

    assert_eq!("synced:payload", output.transformed_message);
    worker_handle.abort()
}

#[tokio::test]
async fn test_concurrency_cancel_newest() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    let task = hatchet
        .task(
            "conc-cancel-step",
            async move |_input: SimpleInput,
                        _ctx: hatchet_sdk::Context|
                        -> anyhow::Result<SimpleOutput> {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                Ok(SimpleOutput {
                    transformed_message: format!("done"),
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

    let task_clone = task.clone();
    let hatchet_clone = hatchet.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet_sdk::worker::worker::WorkerBuilder::default()
            .name(String::from("conc-worker"))
            .client(hatchet_clone)
            .build()
            .unwrap()
            .add_task_or_workflow(&task_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Run first task
    let _run1 = task
        .run_no_wait(
            &SimpleInput { message: "payload".to_string() },
            None,
        )
        .await
        .unwrap();
        
    // Wait slightly to let it transition to running
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Run second task, which should exceed max_runs and be cancelled
    let run2 = task
        .run_no_wait(
            &SimpleInput { message: "payload".to_string() },
            None,
        )
        .await
        .unwrap();
        
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let res2 = hatchet.workflow_rest_client.get(&run2).await.unwrap();
    let status2 = format!("{:?}", res2.run.status);
    println!("STATUS2: {}, RUN2 RESULT: {:?}", status2, res2);
    assert!(status2 == "Cancelled" || status2 == "Failed", "Status was {}", status2);
    worker_handle.abort()
}

/// Verifies that concurrency expressions defined on a Task are correctly
/// hoisted to the Workflow when the task is added via `Workflow::add_task()`.
///
/// This mirrors `test_concurrency_cancel_newest` but wraps the task in a
/// Workflow. Before the hoisting fix, concurrency would have been silently
/// dropped and the second run would NOT be cancelled.
#[tokio::test]
async fn test_workflow_hoists_task_concurrency() {
    let (_postgres, hatchet_container, token) = common::start_containers_and_get_token().await;
    let server_url = format!(
        "http://localhost:{}",
        hatchet_container.get_host_port_ipv4(8888).await.unwrap()
    );
    let grpc_broadcast_address = format!(
        "localhost:{}",
        hatchet_container.get_host_port_ipv4(7077).await.unwrap()
    );
    let hatchet = Hatchet::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
        .await
        .unwrap();

    // Define a task with CancelNewest concurrency (max_runs: 1).
    let task = hatchet
        .task(
            "wf-conc-step",
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
    let workflow = hatchet
        .workflow::<SimpleInput, serde_json::Value>("wf-conc-test")
        .build()
        .unwrap()
        .add_task(&task);

    let workflow_clone = workflow.clone();
    let hatchet_clone = hatchet.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet_sdk::worker::worker::WorkerBuilder::default()
            .name(String::from("wf-conc-worker"))
            .client(hatchet_clone)
            .build()
            .unwrap()
            .add_task_or_workflow(&workflow_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register workflow
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Run first workflow — should start executing
    let _run1 = workflow
        .run_no_wait(
            &SimpleInput { message: "payload".to_string() },
            None,
        )
        .await
        .unwrap();

    // Wait slightly to let it transition to running
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Run second workflow — should exceed max_runs and be cancelled
    let run2 = workflow
        .run_no_wait(
            &SimpleInput { message: "payload".to_string() },
            None,
        )
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let res2 = hatchet.workflow_rest_client.get(&run2).await.unwrap();
    let status2 = format!("{:?}", res2.run.status);
    println!("STATUS2: {}, RUN2 RESULT: {:?}", status2, res2);
    assert!(status2 == "Cancelled" || status2 == "Failed", "Status was {}", status2);
    worker_handle.abort()
}
