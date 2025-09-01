use anyhow;
use hatchet_sdk::{Context, EmptyModel, Hatchet};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let hatchet = Hatchet::from_env().await.unwrap();

    let first_task = hatchet.task(
        "first_task",
        async move |_input: EmptyModel, _ctx: Context| -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({"output": "Hello World"}))
        },
    );

    let second_task = hatchet
        .task(
            "second_task",
            async move |_input: EmptyModel, ctx: Context| -> anyhow::Result<serde_json::Value> {
                let first_result = ctx.parent_output("first_task").await?;
                println!(
                    "First task said: {}",
                    first_result.get("output").unwrap().to_string()
                );
                Ok(serde_json::json!({"final_result": "Completed"}))
            },
        )
        .add_parent(&first_task);

    let mut workflow = hatchet
        .workflow::<EmptyModel, serde_json::Value>()
        .name(String::from("dag-workflow"))
        .build()
        .add_task(first_task)
        .unwrap()
        .add_task(second_task)
        .unwrap();

    workflow.run_no_wait(EmptyModel, None).await.unwrap();
}
