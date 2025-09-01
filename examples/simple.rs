use hatchet_sdk::{Context, Hatchet, Register, Runnable};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let hatchet = Hatchet::from_env().await.unwrap();

    #[derive(Clone, Serialize, Deserialize)]
    struct SimpleInput {
        message: String,
    }

    #[derive(Clone, Serialize, Deserialize)]
    struct SimpleOutput {
        message: String,
    }

    let mut task = hatchet
        .task(
            "simple-task",
            async move |input: SimpleInput,
                        ctx: Context|
                        -> Result<SimpleOutput, hatchet_sdk::HatchetError> {
                ctx.log("Starting simple task").await?;
                Ok(SimpleOutput {
                    message: input.message.to_lowercase(),
                })
            },
        )
        .build()
        .unwrap();

    let hatchet_clone = hatchet.clone();
    let task_clone = task.clone();

    let worker_handle = tokio::spawn(async move {
        hatchet_clone
            .worker()
            .name(String::from("test-worker"))
            .max_runs(5)
            .build()
            .unwrap()
            .add_task_or_workflow(task_clone)
            .start()
            .await
            .unwrap()
    });

    // Wait for the worker to register the workflow with Hatchet
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let result = task
        .run(
            SimpleInput {
                message: String::from("Hello, world!"),
            },
            None,
        )
        .await
        .unwrap();
    println!(
        "First task result: {}",
        serde_json::to_string(&result).unwrap()
    );

    worker_handle.abort();
}
