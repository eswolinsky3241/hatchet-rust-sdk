use hatchet_sdk::serde::{Deserialize, Serialize};
use hatchet_sdk::{
    ConcurrencyExpression, ConcurrencyLimitStrategy, Context, Hatchet, RateLimit,
    RateLimitDuration, Runnable, Register, tokio,
};

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "hatchet_sdk::serde")]
pub struct ProviderInput {
    pub provider_id: String,
    pub payload: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "hatchet_sdk::serde")]
pub struct ProviderOutput {
    pub result: String,
}

/// Demonstrates the "Double Lock" pattern:
/// - **Concurrency control** (workflow-level): limits in-flight runs per provider.
/// - **Dynamic rate limiting** (task-level): caps throughput per provider per minute.
pub async fn create_flow_control_task() -> hatchet_sdk::Task<ProviderInput, ProviderOutput> {
    let hatchet = Hatchet::from_env().await.unwrap();

    hatchet
        .task(
            "provider-sync",
            async move |input: ProviderInput,
                        ctx: Context|
                        -> hatchet_sdk::anyhow::Result<ProviderOutput> {
                ctx.log(&format!("Syncing provider {}", input.provider_id))
                    .await?;
                Ok(ProviderOutput {
                    result: format!("synced:{}", input.payload),
                })
            },
        )
        // Concurrency: at most 2 concurrent runs per provider_id
        .concurrency(vec![ConcurrencyExpression {
            expression: "input.provider_id".to_string(),
            max_runs: 2,
            limit_strategy: ConcurrencyLimitStrategy::GroupRoundRobin,
        }])
        // Rate limit: at most 10 units per minute per provider, consuming 1 unit per run
        .rate_limits(vec![RateLimit::Dynamic {
            key: "provider-limit".to_string(),
            key_expr: "input.provider_id".to_string(),
            units: 1,
            limit: 10,
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

    let input = ProviderInput {
        provider_id: "acme-corp".to_string(),
        payload: "important-data".to_string(),
    };

    let worker_task = task.clone();
    let hatchet_clone = Hatchet::from_env().await.unwrap();
    let worker_handle = tokio::spawn(async move {
        hatchet_clone
            .worker("flow-control-worker")
            .build()
            .unwrap()
            .add_task_or_workflow(&worker_task)
            .start()
            .await
            .unwrap();
    });

    println!("Waiting for worker to register task...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Triggering workflow run...");
    let result = task.run(&input, None).await.unwrap();
    println!("Result: {}", result.result);
    
    worker_handle.abort();
}
