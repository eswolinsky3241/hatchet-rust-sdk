use hatchet_sdk::{Context, Hatchet, Register, Runnable};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let hatchet = Hatchet::from_env().await.unwrap();

    // Define your input and output types
    #[derive(Clone, Serialize, Deserialize)]
    struct SimpleInput {
        message: String,
    }

    #[derive(Clone, Serialize, Deserialize)]
    struct SimpleOutput {
        transformed_message: String,
    }

    let mut task = hatchet
        .task(
            "simple-task",
            async move |input: SimpleInput,
                        ctx: Context|
                        -> Result<SimpleOutput, hatchet_sdk::HatchetError> {
                ctx.log("Starting simple task").await?;
                Ok(SimpleOutput {
                    transformed_message: input.message.to_lowercase(),
                })
            },
        )
        .build()
        .unwrap();

    let hatchet_clone = hatchet.clone();
    let task_clone = task.clone();

    // Spawn a worker to run the task in a separate thread
    let worker_handle = tokio::spawn(async move {
        let mut worker = hatchet_clone
            .worker("simple-worker")
            .max_runs(5)
            .build()
            .unwrap()
            .add_task_or_workflow(task_clone);

        worker.start().await.unwrap()
    });

    // Wait for the worker to register the workflow with Hatchet
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Run the task synchronously
    let input = SimpleInput {
        message: String::from("Hello, world!"),
    };
    let result = task.run(input, None).await.unwrap();
    println!("Result: {}", result.transformed_message);

    worker_handle.abort();
}
