pub mod client;
pub(crate) mod clients;
pub mod config;
pub mod error;
pub(crate) mod grpc;
pub mod models;
pub(crate) mod rest;
pub mod tasks;
pub mod utils;
pub mod worker;
pub mod workflow;

use std::cell::RefCell;

pub use client::HatchetClient;
pub use error::HatchetError;
pub use tasks::Context;
pub use worker::Worker;

#[derive(Clone)]
pub struct TaskContext {
    pub workflow_run_id: String,
    pub step_run_id: String,
    pub worker_id: String,
    pub child_index: i32,
}

tokio::task_local! {
    static TASK_CONTEXT: RefCell<TaskContext>;
}
