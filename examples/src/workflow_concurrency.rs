use hatchet_sdk::serde::{Deserialize, Serialize};
use hatchet_sdk::{
    ConcurrencyExpression, ConcurrencyLimitStrategy, Context, Hatchet, Runnable, Register, tokio,
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

/// Demonstrates concurrency defined on a **Task** that is then added to a
/// **Workflow**. Hatchet evaluates concurrency at the workflow level, so the
/// SDK must hoist task-level concurrency expressions up to the workflow when
/// `Workflow::add_task()` is called.
///
/// Without the hoisting fix, these concurrency rules would be silently dropped
/// and all 20 runs would execute without any concurrency limit.
#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

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
    // should be hoisted to the workflow level automatically.
    let workflow = hatchet
        .workflow::<TestInput, serde_json::Value>("workflow-concurrency-test")
        .build()
        .unwrap()
        .add_task(&task);

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

    println!("Starting worker...");
    Hatchet::from_env()
        .await
        .unwrap()
        .worker("workflow-concurrency-worker")
        .build()
        .unwrap()
        .add_task_or_workflow(&workflow)
        .start()
        .await
        .unwrap();
}
