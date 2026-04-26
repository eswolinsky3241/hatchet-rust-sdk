use hatchet_sdk::serde::{Deserialize, Serialize};
use hatchet_sdk::{Context, EmptyModel, Hatchet, Runnable, anyhow, serde_json, tokio};

#[derive(Serialize, Deserialize)]
#[serde(crate = "hatchet_sdk::serde")]
struct FirstTaskOutput {
    output: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "hatchet_sdk::serde")]
struct SecondTaskOutput {
    first_step_result: String,
    final_result: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "hatchet_sdk::serde")]
pub struct WorkflowOutput {
    first_task: FirstTaskOutput,
    second_task: SecondTaskOutput,
}

pub async fn create_dag_workflow() -> hatchet_sdk::Workflow<EmptyModel, WorkflowOutput> {
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

    hatchet
        .workflow::<EmptyModel, WorkflowOutput>("dag-workflow")
        .build()
        .unwrap()
        .add_task(&first_task)
        .add_task(&second_task)
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();
    let workflow = create_dag_workflow().await;

    let result = workflow.run(&EmptyModel, None).await.unwrap();
    println!(
        "First task result: {}",
        serde_json::to_string(&result.first_task).unwrap()
    );
    println!(
        "Second task result: {}",
        serde_json::to_string(&result.second_task).unwrap()
    );
}
