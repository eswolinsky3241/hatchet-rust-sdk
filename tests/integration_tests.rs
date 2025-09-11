use hatchet_sdk::worker::worker::WorkerBuilder;
use hatchet_sdk::{Hatchet, HatchetError, Register, Runnable};

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

    let options = TriggerWorkflowOptionsBuilder::default()
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
