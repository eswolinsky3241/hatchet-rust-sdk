use anyhow;
use hatchet_sdk::{Context, Hatchet};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let hatchet = Hatchet::from_env().await.unwrap();

    #[derive(Serialize, Deserialize)]
    struct SimpleInput {
        message: String,
    }

    #[derive(Serialize, Deserialize)]
    struct SimpleOutput {
        message: String,
    }

    let task = hatchet.task(
        "simple-task",
        async move |input: SimpleInput, ctx: Context| -> anyhow::Result<SimpleOutput> {
            ctx.log("Starting simple task").await?;
            Ok(SimpleOutput {
                message: input.message.to_lowercase(),
            })
        },
    );

    let mut workflow = hatchet
        .workflow::<SimpleInput, SimpleOutput>()
        .name(String::from("simple-workflow"))
        .build()
        .unwrap()
        .add_task(task)
        .unwrap();

    hatchet
        .worker()
        .name(String::from("simple-worker"))
        .max_runs(5)
        .build()
        .unwrap()
        .add_workflow(workflow)
        .start()
        .await
        .unwrap();
}
