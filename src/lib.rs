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
//! ```compile_fail
//! let mut worker = hatchet_clone
//!     .worker("simple-worker")
//!     .build()
//!     .unwrap()
//!     .add_task_or_workflow(simple_task);
//! worker.start().await.unwrap()
//! ```
//! ## Declarative Workflow Design (DAGs)
//! Hatchet workflows are designed in a Directed Acyclic Graph (DAG) format,
//! where each task is a node in the graph, and the dependencies between tasks are the edges.
//! ### Defining a Workflow
//! Start by declaring a workflow with a name:
//! ```
//! use hatchet_sdk::{Context, EmptyModel, Hatchet, Register, Runnable};
//! use serde::{Deserialize, Serialize};
//!
//! #[tokio::main]
//! async fn main() {
//!     let hatchet = Hatchet::from_env().unwrap();
//!
//!     simple = hatchet.workflow::<EmptyModel, serde_json::Value>("SimpleWorkflow")
//!         .build()
//!         .unwrap();
//! ```
//! ### Building a DAG with Task Dependencies
//! The power of Hatchet’s workflow design comes from connecting tasks into a DAG structure.
//! Tasks can specify dependencies (parents) which must complete successfully before the task can start.
//! ```
//!     #[derive(Serialize, Deserialize)]
//!     struct FirstTaskOutput {
//!         output: String,
//!     }
//!
//!     let first_task = hatchet
//!         .task(
//!          "first_task",
//!          async move |_input: EmptyModel, _ctx: Context| -> anyhow::Result<FirstTaskOutput> {
//!              Ok(FirstTaskOutput {
//!                  output: "Hello World".to_string(),
//!              })
//!          },
//!      )
//!      .build()
//!      .unwrap();
//!
//!     #[derive(Serialize, Deserialize)]
//!     struct SecondTaskOutput {
//!         first_task_result: String,
//!         final_result: String
//!     }
//!
//!     let second_task = hatchet
//!         .task(
//!            "second_task",
//!            async move |_input: EmptyModel, ctx: Context| -> anyhow::Result<SecondTaskOutput> {
//!                let first_result = ctx.parent_output("first_task").await?;
//!                Ok(SecondTaskOutput {
//!                    first_step_result: first_result.get("output").unwrap().to_string(),
//!                    final_result: "Completed".to_string(),
//!                })
//!             },
//!         )
//!    .build()
//!    .unwrap()
//!    .add_parent(&first_task);
//! ```
//! ### Running a Workflow
//! You can run workflows directly or enqueue them for asynchronous execution.
//! ```
//! let result = workflow.run(EmptyModel, None).await.unwrap();
//! let run_id: String = workflow.run_no_wait(EmptyModel, None).await.unwrap();
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
pub use runnables::{Runnable, Task, TriggerWorkflowOptions, Workflow};
pub use utils::EmptyModel;
pub use worker::{Register, Worker};
