use hatchet_sdk::HatchetClient;
use hatchet_sdk::HatchetError;
use hatchet_sdk::Worker;
use serde::{Deserialize, Serialize};

mod common;

#[tokio::test]
async fn test_hatchet() {
    let (_, hatchet_container, token) = common::start_containers_and_get_token().await;
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

    #[derive(Deserialize, Serialize, Clone)]
    struct SimpleInput {
        message: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct SimpleOutput {
        transformed_message: String,
    }

    use thiserror::Error;
    #[derive(Debug, Error)]
    pub enum MyError {
        #[error("Test failed.")]
        Failure(#[from] HatchetError),
    }

    let my_task = hatchet.new_task(
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
        .new_workflow::<SimpleInput, SimpleOutput>("rust-workflow3", vec![], vec![], vec![])
        .add_task(my_task)
        .unwrap();

    let workflow_clone = workflow.clone();
    let worker_handle = tokio::spawn(async move {
        Worker::new("rust-worker", hatchet.clone(), 5)
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
