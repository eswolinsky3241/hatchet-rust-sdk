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

pub async fn create_concurrency_task() -> hatchet_sdk::Task<TestInput, TestOutput> {
    Hatchet::from_env().await.unwrap()
        .task(
            "concurrency_test",
            async move |input: TestInput, ctx: Context| -> hatchet_sdk::anyhow::Result<TestOutput> {
                ctx.log(&format!("Starting concurrency test {} for {}", input.index, input.provider_id)).await?;
                tokio::time::sleep(Duration::from_secs(input.delay_seconds)).await;
                Ok(TestOutput { result: "done".into() })
            },
        )
        .concurrency(vec![ConcurrencyExpression {
            expression: "input.provider_id".to_string(),
            max_runs: 2,
            limit_strategy: ConcurrencyLimitStrategy::GroupRoundRobin,
        }])
        .build()
        .unwrap()
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

    let task = create_concurrency_task().await;

    println!("Sending 20 events for concurrency test...");
    for i in 0..20 {
        let input = TestInput {
            provider_id: "acme-corp".to_string(),
            index: i,
            delay_seconds: 15,
        };

        task.run_no_wait(&input, None).await.unwrap();
    }

    println!("\n========================================");
    println!("          ALL TASKS QUEUED!             ");
    println!("   Go observe them in the Hatchet UI!   ");
    println!("========================================\n");
}
