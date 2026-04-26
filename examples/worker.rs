use hatchet_sdk::{Hatchet, Register, tokio};

#[path = "simple.rs"]
mod simple;
use simple::create_simple_task;

#[path = "dag.rs"]
mod dag;
use dag::create_dag_workflow;

#[path = "error.rs"]
mod error;
use error::create_error_task;

#[path = "dynamic_child_spawning.rs"]
mod dynamic_child_spawning;
use dynamic_child_spawning::create_child_spawning_workflow;

#[path = "input_json_schema.rs"]
mod input_json_schema;
use input_json_schema::create_schema_workflow;

#[path = "streaming.rs"]
mod streaming;
use streaming::create_streaming_task;

#[path = "concurrency.rs"]
mod concurrency;
use concurrency::create_concurrency_task;

#[path = "rate_limits.rs"]
mod rate_limits;
use rate_limits::create_rate_limit_task;

#[path = "flow_control.rs"]
mod flow_control;
use flow_control::create_flow_control_task;

#[path = "workflow_concurrency.rs"]
mod workflow_concurrency;
use workflow_concurrency::create_workflow_concurrency;

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();
    env_logger::Builder::new()
        .filter_module("hatchet_sdk", log::LevelFilter::Debug)
        .init();

    let hatchet = Hatchet::from_env().await.unwrap();

    let simple_task = create_simple_task().await;
    let dag_workflow = create_dag_workflow().await;
    let error_task = create_error_task().await;
    let (parent_workflow, child_workflow) = create_child_spawning_workflow().await;
    let schema_workflow = create_schema_workflow().await;
    let streaming_task = create_streaming_task().await;
    let concurrency_task = create_concurrency_task().await;
    let rate_limit_task = create_rate_limit_task().await;
    let flow_control_task = create_flow_control_task().await;
    let workflow_concurrency = create_workflow_concurrency().await;

    hatchet
        .worker("example-worker")
        .build()
        .unwrap()
        .add_task_or_workflow(&simple_task)
        .add_task_or_workflow(&dag_workflow)
        .add_task_or_workflow(&error_task)
        .add_task_or_workflow(&parent_workflow)
        .add_task_or_workflow(&child_workflow)
        .add_task_or_workflow(&schema_workflow)
        .add_task_or_workflow(&streaming_task)
        .add_task_or_workflow(&concurrency_task)
        .add_task_or_workflow(&rate_limit_task)
        .add_task_or_workflow(&flow_control_task)
        .add_task_or_workflow(&workflow_concurrency)
        .start()
        .await
        .unwrap();
}
