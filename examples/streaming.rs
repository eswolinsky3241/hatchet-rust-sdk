use futures::StreamExt;
use hatchet_sdk::{Context, Hatchet, Runnable, TriggerWorkflowOptionsBuilder};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct StreamInput {
    pub message: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StreamOutput {
    pub chunks_sent: usize,
}

pub async fn create_streaming_task() -> hatchet_sdk::Task<StreamInput, StreamOutput> {
    async fn streaming_task_func(input: StreamInput, ctx: Context) -> anyhow::Result<StreamOutput> {
        ctx.log("Starting streaming task").await?;

        let chunks = vec![
            format!("Processing: {}", input.message),
            "Chunk 1: Hello".to_string(),
            "Chunk 2: World".to_string(),
            "Chunk 3: Done".to_string(),
        ];

        for chunk in &chunks {
            ctx.put_stream(chunk.as_bytes().to_vec()).await?;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        Ok(StreamOutput {
            chunks_sent: chunks.len(),
        })
    }

    let hatchet: Hatchet = Hatchet::from_env().await.unwrap();

    hatchet
        .task("streaming-task", streaming_task_func)
        .build()
        .unwrap()
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

    let task = create_streaming_task().await;

    let options = TriggerWorkflowOptionsBuilder::default()
        .additional_metadata(Some(serde_json::json!({
            "environment": "dev",
        })))
        .build()
        .unwrap();

    // Trigger the task without waiting for completion
    let run_id = task
        .run_no_wait(
            &StreamInput {
                message: "Hello, streaming!".to_string(),
            },
            Some(&options),
        )
        .await
        .unwrap();
    println!("Run ID: {}", run_id);

    // Subscribe to stream events immediately after getting the run ID.
    // The task hasn't been scheduled to a worker yet, so we won't miss any events.
    let mut hatchet = Hatchet::from_env().await.unwrap();
    let mut stream = hatchet
        .workflow_rest_client
        .subscribe_to_stream(&run_id)
        .await
        .unwrap();

    println!("Subscribed to stream, waiting for chunks...");

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                let text = String::from_utf8_lossy(&data);
                println!("Stream chunk: {}", text);
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }

    println!("Stream ended.");
}
