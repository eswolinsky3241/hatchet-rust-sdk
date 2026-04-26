use hatchet_sdk::serde::{Deserialize, Serialize};
use hatchet_sdk::{
    ConcurrencyExpression, ConcurrencyLimitStrategy, Context, Hatchet, Runnable, tokio,
};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "hatchet_sdk::serde")]
pub struct TestInput {
    pub provider_id: String,
    pub index: i32,
    pub delay_seconds: u64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "hatchet_sdk::serde")]
pub struct TestOutput {
    pub result: String,
}

/// Returns a workflow that demonstrates concurrency defined on a **Task** being
/// hoisted to the **Workflow** level when `Workflow::add_task()` is called.
///
/// Without the hoisting fix, these concurrency rules would be silently dropped
/// and all 20 runs would execute without any concurrency limit.
pub async fn create_workflow_concurrency() -> hatchet_sdk::Workflow<TestInput, serde_json::Value> {
    let hatchet = Hatchet::from_env().await.unwrap();

    // Define a task with concurrency — max 2 concurrent runs per provider_id.
    let task = hatchet
        .task(
            "wf_conc_step",
            async move |input: TestInput, ctx: Context| -> hatchet_sdk::anyhow::Result<TestOutput> {
                ctx.log(&format!(
                    "Starting workflow concurrency test {} for {}",
                    input.index, input.provider_id
                ))
                .await?;
                tokio::time::sleep(Duration::from_secs(input.delay_seconds)).await;
                Ok(TestOutput {
                    result: "done".into(),
                })
            },
        )
        .concurrency(vec![ConcurrencyExpression {
            expression: "input.provider_id".to_string(),
            max_runs: 2,
            limit_strategy: ConcurrencyLimitStrategy::GroupRoundRobin,
        }])
        .build()
        .unwrap();

    // Wrap the task inside a Workflow. The task's concurrency expressions
    // are hoisted to the workflow level automatically.
    hatchet
        .workflow::<TestInput, serde_json::Value>("workflow-concurrency-test")
        .build()
        .unwrap()
        .add_task(&task)
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

    let workflow = create_workflow_concurrency().await;

    println!("Sending 20 events for workflow concurrency test...");
    for i in 0..20 {
        let input = TestInput {
            provider_id: "acme-corp".to_string(),
            index: i,
            delay_seconds: 15,
        };

        workflow.run_no_wait(&input, None).await.unwrap();
    }

    println!("\n========================================");
    println!("       ALL WORKFLOW RUNS QUEUED!        ");
    println!("   Go observe them in the Hatchet UI!   ");
    println!("========================================\n");
}
