use hatchet_sdk::HatchetClient;
use hatchet_sdk::HatchetError;
use hatchet_sdk::Worker;
use hatchet_sdk::worker::worker::WorkerBuilder;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod common;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SimpleInput {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SimpleOutput {
    pub transformed_message: String,
}

#[derive(Debug, Error)]
pub enum MyError {
    #[error("Test failed.")]
    Failure,
}

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
    let hatchet =
        HatchetClient::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
            .await
            .unwrap();

    use thiserror::Error;
    #[derive(Debug, Error)]
    pub enum MyError {
        #[error("Test failed.")]
        Failure(#[from] HatchetError),
    }

    let my_task = hatchet.task(
        "step1",
        async move |input: SimpleInput,
                    _ctx: hatchet_sdk::Context|
                    -> Result<SimpleOutput, MyError> {
            Ok(SimpleOutput {
                transformed_message: input.message.to_lowercase(),
            })
        },
    );
    let mut workflow = hatchet
        .workflow::<SimpleInput, SimpleOutput>()
        .name(String::from("rust-workflow3"))
        .build()
        .unwrap()
        .add_task(my_task)
        .unwrap();

    let workflow_clone = workflow.clone();
    let worker_handle = tokio::spawn(async move {
        WorkerBuilder::default()
            .name(String::from("rust-worker"))
            .client(hatchet.clone())
            .max_runs(5)
            .build()
            .unwrap()
            .add_workflow(workflow_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    assert_eq!(
        "uppercase",
        workflow
            .run(
                SimpleInput {
                    message: "UPPERCASE".to_string()
                },
                None
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
    let hatchet =
        HatchetClient::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
            .await
            .unwrap();

    let my_task = hatchet.task(
        "step1",
        async move |_input: SimpleInput,
                    _ctx: hatchet_sdk::Context|
                    -> Result<SimpleOutput, MyError> { Err(MyError::Failure) },
    );
    let mut workflow = hatchet
        .workflow::<SimpleInput, SimpleOutput>()
        .name(String::from("rust-workflow3"))
        .build()
        .unwrap()
        .add_task(my_task)
        .unwrap();

    let workflow_clone = workflow.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet
            .worker()
            .name(String::from("rust-worker"))
            .max_runs(5)
            .build()
            .unwrap()
            .add_workflow(workflow_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let output = workflow
        .run(
            SimpleInput {
                message: "UPPERCASE".to_string(),
            },
            None,
        )
        .await;

    let _err = HatchetError::WorkflowFailed {
        error_message: "Task execution failed: Test failed".to_string(),
    };

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
    let hatchet =
        HatchetClient::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
            .await
            .unwrap();

    let child_task = hatchet.task(
        "child_task",
        async move |_input: hatchet_sdk::EmptyModel,
                    _ctx: hatchet_sdk::Context|
                    -> Result<serde_json::Value, MyError> {
            Ok(serde_json::json!({"output": "Hello from child task"}))
        },
    );

    let mut child_workflow = hatchet
        .workflow::<hatchet_sdk::EmptyModel, serde_json::Value>()
        .name(String::from("child_workflow"))
        .build()
        .unwrap()
        .add_task(child_task)
        .unwrap();

    let child_workflow_clone = child_workflow.clone();

    let parent_task = hatchet.task(
        "parent_task",
        async move |_input: hatchet_sdk::EmptyModel,
                    _ctx: hatchet_sdk::Context|
                    -> Result<serde_json::Value, MyError> {
            Ok(child_workflow
                .run(hatchet_sdk::EmptyModel, None)
                .await
                .unwrap())
        },
    );
    let mut parent_workflow = hatchet
        .workflow::<hatchet_sdk::EmptyModel, serde_json::Value>()
        .name(String::from("parent-workflow"))
        .build()
        .unwrap()
        .add_task(parent_task)
        .unwrap();

    let parent_workflow_clone = parent_workflow.clone();
    let worker_handle = tokio::spawn(async move {
        Worker::new("rust-worker", hatchet.clone(), 5)
            .unwrap()
            .add_workflow(parent_workflow_clone)
            .add_workflow(child_workflow_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let output = parent_workflow
        .run(hatchet_sdk::EmptyModel, None)
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
    let hatchet =
        HatchetClient::from_token(&server_url, &grpc_broadcast_address, token.trim(), "none")
            .await
            .unwrap();

    let parent_task = hatchet.task(
        "parent_task",
        async move |_input: hatchet_sdk::EmptyModel,
                    _ctx: hatchet_sdk::Context|
                    -> Result<serde_json::Value, MyError> {
            Ok(serde_json::json!({"message": "I am your father"}))
        },
    );

    let child_task = hatchet
        .task(
            "child_task",
            async move |_input: hatchet_sdk::EmptyModel,
                        ctx: hatchet_sdk::Context|
                        -> Result<serde_json::Value, MyError> {
                let parent_output = ctx.parent_output("parent_task").await.unwrap();
                let message = parent_output.get("message").unwrap();
                Ok(serde_json::json!({"output": format!("Parent said: {}", message.to_string())}))
            },
        )
        .add_parent(&parent_task);

    let mut dag_workflow = hatchet
        .workflow::<hatchet_sdk::EmptyModel, serde_json::Value>()
        .name(String::from("parent-workflow"))
        .build()
        .unwrap()
        .add_task(parent_task)
        .unwrap()
        .add_task(child_task)
        .unwrap();

    let dag_workflow_clone = dag_workflow.clone();
    let worker_handle = tokio::spawn(async move {
        Worker::new("rust-worker", hatchet.clone(), 5)
            .unwrap()
            .add_workflow(dag_workflow_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register task
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let output = dag_workflow.run(hatchet_sdk::EmptyModel, None).await;

    assert_eq!(
        "Parent said: \"I am your father\"",
        output.unwrap().get("output").unwrap()
    );
    worker_handle.abort()
}
