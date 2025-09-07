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

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();
    let hatchet = Hatchet::from_env().await.unwrap();

    let simple_task = create_simple_task().await;
    let dag_workflow = create_dag_workflow().await;
    let error_task = create_error_task().await;
    let (parent_workflow, child_workflow) = create_child_spawning_workflow().await;

    hatchet
        .worker("example-worker")
        .build()
        .unwrap()
        .add_task_or_workflow(&simple_task)
        .add_task_or_workflow(&dag_workflow)
        .add_task_or_workflow(&error_task)
        .add_task_or_workflow(&parent_workflow)
        .add_task_or_workflow(&child_workflow)
        .start()
        .await
        .unwrap();
}
