use hatchet_sdk::{Context, Hatchet, Runnable, TriggerWorkflowOptions};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ParentInput {
    n: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChildInput {
    a: String,
}

pub async fn create_child_spawning_workflow() -> (
    hatchet_sdk::Workflow<ParentInput, serde_json::Value>,
    hatchet_sdk::Workflow<ChildInput, serde_json::Value>,
) {
    let hatchet = Hatchet::from_env().await.unwrap();

    let child_task_1: hatchet_sdk::Task<ChildInput, serde_json::Value> = hatchet
        .task(
            "child_task_1",
            async move |input: ChildInput, _ctx: Context| -> anyhow::Result<serde_json::Value> {
                println!("child process {}", input.a);
                Ok(serde_json::json!({"status": input.a}))
            },
        )
        .build()
        .unwrap();

    let child_task_2: hatchet_sdk::Task<ChildInput, serde_json::Value> = hatchet
        .task(
            "child_task_2",
            async move |_input: ChildInput, ctx: Context| -> anyhow::Result<serde_json::Value> {
                let process_output = ctx.parent_output("child_task_1").await?;
                let a = process_output.get("status").unwrap();
                Ok(serde_json::json!({"status2": format!("{}2", a.to_string())}))
            },
        )
        .build()
        .unwrap()
        .add_parent(&child_task_1);

    let child_workflow = hatchet
        .workflow::<ChildInput, serde_json::Value>("fanout-child")
        .build()
        .unwrap()
        .add_task(&child_task_1)
        .add_task(&child_task_2);

    let child_workflow_clone = child_workflow.clone();

    let spawn_task = hatchet
        .task(
            "spawn_task",
            async move |input: ParentInput, _ctx: Context| -> anyhow::Result<serde_json::Value> {
                let mut child_tasks = vec![];
                for i in 0..input.n {
                    let mut workflow_clone = child_workflow_clone.clone();
                    let mut options = TriggerWorkflowOptions::default();
                    options.additional_metadata = Some(serde_json::json!({
                        "child_index": i.to_string(),
                    }));
                    let handle = async move {
                        let result = workflow_clone
                            .run(ChildInput { a: i.to_string() }, Some(options))
                            .await
                            .unwrap()
                            .get("child_task_2")
                            .unwrap()
                            .to_owned();
                        result
                    };
                    child_tasks.push(handle);
                }
                let results = futures::future::join_all(child_tasks).await;
                Ok(serde_json::Value::Array(results))
            },
        )
        .build()
        .unwrap();

    let parent_workflow = hatchet
        .workflow::<ParentInput, serde_json::Value>("fanout-parent")
        .build()
        .unwrap()
        .add_task(&spawn_task);

    (parent_workflow, child_workflow)
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

    let (mut parent_workflow, _child_workflow) = create_child_spawning_workflow().await;

    let input = ParentInput { n: 10 };
    let result = parent_workflow.run(input, None).await.unwrap();
    println!("Result: {}", result.to_string());
}
