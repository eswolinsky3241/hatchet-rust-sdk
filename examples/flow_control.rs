use hatchet_sdk::serde::{Deserialize, Serialize};
use hatchet_sdk::{
    ConcurrencyExpression, ConcurrencyLimitStrategy, Context, Hatchet, RateLimit,
    RateLimitDuration, Runnable, tokio,
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

pub async fn create_flow_control_task() -> hatchet_sdk::Task<TestInput, TestOutput> {
    Hatchet::from_env().await.unwrap()
        .task(
            "flow_control_test",
            async move |input: TestInput, ctx: Context| -> hatchet_sdk::anyhow::Result<TestOutput> {
                ctx.log(&format!("Starting flow control test {} for {}", input.index, input.provider_id)).await?;
                tokio::time::sleep(Duration::from_secs(input.delay_seconds)).await;
                Ok(TestOutput { result: "done".into() })
            },
        )
        .concurrency(vec![ConcurrencyExpression {
            expression: "input.provider_id".to_string(),
            max_runs: 2,
            limit_strategy: ConcurrencyLimitStrategy::GroupRoundRobin,
        }])
        .rate_limits(vec![RateLimit::Dynamic {
            key: "provider-flow-rate-limit".to_string(),
            key_expr: "input.provider_id".to_string(),
            units: 1,
            units_expr: None,
            limit: 5,
            duration: RateLimitDuration::Minute,
        }])
        .build()
        .unwrap()
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

    let task = create_flow_control_task().await;

    println!("Sending 20 events for flow control test...");
    for i in 0..20 {
        let input = TestInput {
            provider_id: "acme-corp".to_string(),
            index: i,
            delay_seconds: 2,
        };

        task.run_no_wait(&input, None).await.unwrap();
    }

    println!("\n========================================");
    println!("          ALL TASKS QUEUED!             ");
    println!("   Go observe them in the Hatchet UI!   ");
    println!("========================================\n");
}
