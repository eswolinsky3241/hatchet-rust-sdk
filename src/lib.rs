//! # Hatchet SDK for Rust.
//!
//! This is an unofficial Rust SDK for [Hatchet](https://hatchet.run), a distributed, fault-tolerant task queue.
//! This crate allows you to integrate Hatchet into your Rust applications.
//!
//! ## Examples
//!
//! We recommend adding your Hatchet API token to a `.env` file and installing [dotenvy](https://crates.io/crates/dotenvy) to load it in your application.
//!
//! ```compile_fail
//! use hatchet_sdk::{Context, Hatchet, Register, Runnable};
//! use serde::{Deserialize, Serialize};
//!
//! // Define your input and output types
//! #[derive(Serialize, Deserialize)]
//! struct SimpleInput {
//!     message: String,
//! }
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! struct SimpleOutput {
//!     transformed_message: String,
//! }
//!
//! // Define your task handler
//! async fn simple_task(input: SimpleInput, ctx: Context) -> anyhow::Result<SimpleOutput> {
//!     ctx.log("Starting simple task").await?;
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
//!     let hatchet = Hatchet::from_env().await.unwrap();
//!
//!     // Create a workflow
//!     let mut workflow = hatchet.workflow::<SimpleInput, SimpleOutput>()
//!         .name(String::from("simple-workflow"))
//!         .build()
//!         .add_task(hatchet.task("simple-task", simple_task))
//!         .unwrap();
//!
//!     // Create and start a worker, registering the workflow with Hatchet
//!     hatchet.worker()
//!         .name(String::from("simple-worker"))
//!         .max_runs(5)
//!         .build()
//!         .add_task_or_workflow(workflow)
//!         .start()
//!         .await
//!         .unwrap();
//! }
//! ```
//!
//! ### Running workflows
//!
//! Use the `run` method to run the workflow synchronously:
//!
//! ```compile_fail
//! let output = workflow.run(SimpleInput {
//!     message: "Hello, world!".to_string(),
//! }, None).await.unwrap();
//!
//! println!("Output: {:?}", output);
//! ```
//!
//! Use the `run_no_wait` method to run the workflow asynchronously:
//!
//! ```compile_fail
//! workflow.run_no_wait(SimpleInput {
//!     message: "Hello, world!".to_string(),
//! }, None).await.unwrap();
//! ```
//!

pub(crate) mod clients;
pub mod config;
pub mod context;
pub mod error;
pub mod features;
pub mod runnables;
pub mod utils;
pub mod worker;

pub use clients::hatchet::Hatchet;
pub use context::Context;
pub use error::HatchetError;
pub use runnables::Runnable;
pub use runnables::Task;
pub use runnables::TriggerWorkflowOptions;
pub use runnables::Workflow;
pub use utils::EmptyModel;
pub use worker::Register;
pub use worker::Worker;
