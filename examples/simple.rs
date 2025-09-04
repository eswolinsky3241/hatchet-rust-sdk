use hatchet_sdk::{Context, Hatchet, Runnable};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SimpleInput {
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct SimpleOutput {
    pub transformed_message: String,
}

pub async fn create_simple_task() -> hatchet_sdk::Task<SimpleInput, SimpleOutput> {
    async fn simple_task_func(input: SimpleInput, ctx: Context) -> anyhow::Result<SimpleOutput> {
        ctx.log("Starting simple task").await?;
        Ok(SimpleOutput {
            transformed_message: input.message.to_lowercase(),
        })
    }

    let hatchet: Hatchet = Hatchet::from_env().await.unwrap();

    let simple_task: hatchet_sdk::Task<SimpleInput, SimpleOutput> = hatchet
        .task("simple-task", simple_task_func)
        .build()
        .unwrap();

    simple_task
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

    let task = create_simple_task().await;

    let input = SimpleInput {
        message: String::from("Hello, world!"),
    };
    let result = task.run(input, None).await.unwrap();
    println!("Result: {}", result.transformed_message);
}
