use hatchet_sdk::{Context, Hatchet, Register, Runnable};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let hatchet = Hatchet::from_env().await.unwrap();

    // Define your input and output types
    #[derive(Serialize, Deserialize)]
    struct SimpleInput {
        message: String,
    }

    #[derive(Serialize, Deserialize)]
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

    let workflow = hatchet
        .workflow::<SimpleInput, SimpleOutput>("simple-workflow")
        .build()
        .unwrap()
        .add_task(&task)
        .unwrap();

    let workflow2 = hatchet
        .workflow::<SimpleInput, SimpleOutput>("simple-workflow2")
        .build()
        .unwrap()
        .add_task(&task)
        .unwrap();

    // Wait for the worker to register the workflow with Hatchet
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Run the task synchronously
    let input = SimpleInput {
        message: String::from("Hello, world!"),
    };
    let result = task.run(input, None).await.unwrap();
    println!("Result: {}", result.transformed_message);
}
