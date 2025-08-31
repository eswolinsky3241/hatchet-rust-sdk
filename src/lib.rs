//! # Hatchet SDK for Rust.
//!
//! This is an unofficial Rust SDK for [Hatchet](https://hatchet.run), a distributed, fault-tolerant task queue.
//! This crate allows you to integrate Hatchet into your Rust applications.
//!
//! ## Examples
//!
//! We recommend adding your Hatchet API token to a `.env` file and installing dotenvy to load it in your application.
//!
//! ```toml
//! [dependencies]
//! dotenvy = "0.15.7"
//! ```
//!
//! ```rust
//!
//! use anyhow;
//! use hatchet_sdk::{Context, HatchetClient};
//! use serde::{Deserialize, Serialize};
//!
//! // Define your input and output types
//! #[derive(Serialize, Deserialize)]
//! struct SimpleInput {
//!     message: String,
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! struct SimpleOutput {
//!     transformed_message: String,
//! }
//!
//! // Define your task handler
//! async fn simple_task(input: SimpleInput, ctx: Context) -> anyhow::Result<SimpleOutput> {
//!     Ok(SimpleOutput {
//!         transformed_message: input.message.to_lowercase(),
//!     })
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // Load the .env file
//!     dotenvy::dotenv().ok();
//!
//!     // Create a Hatchet client
//!     let hatchet = HatchetClient::from_env().await.unwrap();
//!
//!     // Create a workflow
//!     let mut workflow = hatchet.workflow::<SimpleInput, SimpleOutput>()
//!         .name(String::from("simple-workflow"))
//!         .build()
//!         .unwrap()
//!         .add_task(hatchet.task("simple-task", simple_task))
//!         .unwrap();
//!
//!     // Enqueue the workflow
//!     let output = workflow.run_no_wait(SimpleInput {
//!         message: "Hello, world!".to_string(),
//!     }, None).await.unwrap();
//!
//!
//!     // Create and start a worker
//!     let worker = hatchet.worker()
//!         .name(String::from("simple-worker"))
//!         .max_runs(5)
//!         .build()
//!         .unwrap()
//!         .add_workflow(workflow)
//!         .start()
//!         .await
//!         .unwrap();
//! }
//!
//! ```

pub(crate) mod clients;
pub mod config;
pub mod context;
pub mod error;
pub mod features;
pub mod task;
pub mod utils;
pub mod worker;
pub mod workflow;

pub use clients::client::HatchetClient;
pub use context::Context;
pub use error::HatchetError;
pub use task::Task;
pub use utils::EmptyModel;
pub use worker::Worker;
pub use workflow::TriggerWorkflowOptions;
pub use workflow::Workflow;
