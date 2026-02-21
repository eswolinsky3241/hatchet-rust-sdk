use futures::StreamExt;
use hatchet_sdk::{Context, Hatchet, Register, Runnable, TriggerWorkflowOptionsBuilder};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct StreamInput {
    pub message: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StreamOutput {
    pub chunks_sent: usize,
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

    let hatchet = Hatchet::from_env().await.unwrap();

    let task = hatchet
        .task(
            "streaming-task",
            async move |input: StreamInput, ctx: Context| -> anyhow::Result<StreamOutput> {
                ctx.log("Starting streaming task").await?;

                let chunks = vec![
                    format!("Processing: {}", input.message),
                    "Chunk 1: Hello".to_string(),
                    "Chunk 2: World".to_string(),
                    "Chunk 3: Done".to_string(),
                ];

                for chunk in &chunks {
                    ctx.put_stream(chunk.as_bytes().to_vec()).await?;
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }

                Ok(StreamOutput {
                    chunks_sent: chunks.len(),
                })
            },
        )
        .build()
        .unwrap();

    // Start the worker in the background
    let task_clone = task.clone();
    let hatchet_worker = hatchet.clone();
    let worker_handle = tokio::spawn(async move {
        hatchet_worker
            .worker("streaming-worker")
            .build()
            .unwrap()
            .add_task_or_workflow(&task_clone)
            .start()
            .await
            .unwrap()
    });

    // Give worker time to register
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

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
    let mut hatchet_consumer = hatchet.clone();
    let mut stream = hatchet_consumer
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
    worker_handle.abort();
}
