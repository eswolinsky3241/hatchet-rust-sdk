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
                Ok(serde_json::json!({"first_step_result": first_result.get("output").unwrap(), "final_result": "Completed"}))
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

    let hatchet_clone = hatchet.clone();
    let workflow_clone = workflow.clone();

    let worker_handle = tokio::spawn(async move {
        hatchet_clone
            .worker()
            .name(String::from("test-worker"))
            .max_runs(5)
            .build()
            .add_workflow(workflow_clone)
            .start()
            .await
            .unwrap()
    });

    // Wait for the worker to register the workflow with Hatchet
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let result = workflow.run(EmptyModel, None).await.unwrap();
    println!("Workflow result: {}", result.to_string());
    worker_handle.abort();
}
