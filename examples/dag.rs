use anyhow;
use hatchet_sdk::{Context, EmptyModel, Hatchet, Register, Runnable};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    #[derive(Clone, Serialize, Deserialize, Debug)]
    struct FirstTaskOutput {
        output: String,
    }

    #[derive(Clone, Serialize, Deserialize, Debug)]
    struct SecondTaskOutput {
        first_step_result: String,
        final_result: String,
    }

    #[derive(Clone, Serialize, Deserialize, Debug)]
    struct WorkflowOutput {
        first_task: FirstTaskOutput,
        second_task: SecondTaskOutput,
    }

    dotenvy::dotenv().ok();
    let hatchet = Hatchet::from_env().await.unwrap();

    let first_task = hatchet
        .task(
            "first_task",
            async move |_input: EmptyModel, _ctx: Context| -> anyhow::Result<FirstTaskOutput> {
                Ok(FirstTaskOutput {
                    output: "Hello World".to_string(),
                })
            },
        )
        .build()
        .unwrap();

    let second_task = hatchet
        .task(
            "second_task",
            async move |_input: EmptyModel, ctx: Context| -> anyhow::Result<SecondTaskOutput> {
                let first_result = ctx.parent_output("first_task").await?;
                Ok(SecondTaskOutput {
                    first_step_result: first_result.get("output").unwrap().to_string(),
                    final_result: "Completed".to_string(),
                })
            },
        )
        .build()
        .unwrap()
        .add_parent(&first_task);

    let mut workflow = hatchet
        .workflow::<EmptyModel, WorkflowOutput>()
        .name(String::from("dag-workflow"))
        .build()
        .unwrap()
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
            .unwrap()
            .add_task_or_workflow(workflow_clone)
            .start()
            .await
            .unwrap()
    });

    // Wait for the worker to register the workflow with Hatchet
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let result = workflow.run(EmptyModel, None).await.unwrap();
    println!(
        "First task result: {}",
        serde_json::to_string(&result.first_task).unwrap()
    );
    println!(
        "Second task result: {}",
        serde_json::to_string(&result.second_task).unwrap()
    );
    worker_handle.abort();
}
