//! # Hatchet SDK for Rust.
//!
//! This is an unofficial Rust SDK for [Hatchet](https://hatchet.run), a distributed, fault-tolerant task queue.
//! This crate allows you to integrate Hatchet into your Rust applications.
//!
//! ## Setup
//! We recommend adding your Hatchet API token to a `.env` file and installing [dotenvy](https://crates.io/crates/dotenvy) to load it in your application.
//!
//! ## Declaring Your First Task
//! ### Defining a task
//! Start by declaring a task with a name. The task object can be built with optional configuration options.
//! Tasks have input and output types, which should implement the `Serialize` and `Deserialize` traits from `serde` for JSON serialization and deserialization.
//!
//! ```rust
//! use hatchet_sdk::{Context, Hatchet};
//! use serde::{Deserialize, Serialize};
//!
//! #[tokio::main]
//! async fn main() {
//!     dotenvy::dotenv().ok();
//!     let hatchet = Hatchet::from_env().unwrap();
//!
//!     #[derive(Serialize)]
//!     struct SimpleInput {
//!         message: String,
//!     }
//!
//!     #[derive(Deserialize)]
//!     struct SimpleOutput {
//!         transformed_message: String,
//!     }
//!
//!     let mut simple_task = hatchet.task(
//!             "simple-task",
//!             async move |input: SimpleInput,
//!                         ctx: Context|
//!                         -> Result<SimpleOutput, anyhow::HatchetError> {
//!                 ctx.log("Starting simple task").await?;
//!                 Ok(SimpleOutput {
//!                     transformed_message: input.message.to_lowercase(),
//!                 })
//!             },
//!         )
//!         .build()
//!         .unwrap();
//! }
//!```
//! ### Running a task
//!
//! With your task defined, you can import it wherever you need to use it and invoke it with the run method.
//!
//! <div class="warning">NOTE: You must first register the task on a worker before you can run it.</div>
//!
//! ```compile_fail
//! input = SimpleInput { message: String::from("HeLlO WoRlD")};
//! simple.run(input, None);
//! ```
//! ## Workers
//! Workers are responsible for executing individual tasks.
//! ### Declaring a Worker
//! Declare a worker by calling the worker method on the Hatchet client. Tasks and workflows can be added to the worker. When the worker starts
//! it will register the tasks with the Hatchet engine, allowing them to be triggered and assigned.
//!
//! ```no_compile
//! let mut worker = hatchet_clone
//!     .worker("simple-worker")
//!     .build()
//!     .unwrap()
//!     .add_task_or_workflow(simple_task);
//! worker.start().await.unwrap()
//! ```

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
