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
        .start()
        .await
        .unwrap();
}
