 # 🪓 Hatchet SDK for Rust.

This is an unofficial Rust SDK for [Hatchet](https://hatchet.run), a distributed, fault-tolerant task queue.
This crate allows you to integrate Hatchet into your Rust applications.
## Setup
We recommend adding your Hatchet API token to a `.env` file and installing [dotenvy](https://crates.io/crates/dotenvy) to load it in your application.
## Hatchet Version Compatibility
This library is tested against the following Hatchet versions:
| Version    | Compatible |
| :------: | :-------: |
| v0.67.0  | ❌ |
| v0.68.0  | ✅ |
| v0.69.0  | ✅ |
| v0.70.0  | ✅ |
| v0.71.0  | ✅ |

## Declaring Your First Task
### Defining a task
Start by declaring a task with a name. The task object can be built with optional configuration options.
Tasks have input and output types, which should implement the `Serialize` and `Deserialize` traits from `serde` for JSON serialization and deserialization.

### Running a task
With your task defined, you can import it wherever you need to use it and invoke it with the `run` method.
<div class="warning">NOTE: You must first register the task on a worker before you can run it.</div>

```rust no_run
use hatchet_sdk::{Context, EmptyModel, Hatchet, Runnable};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SimpleInput {
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct SimpleOutput {
    pub transformed_message: String,
}

async fn simple_task_func(input: SimpleInput, ctx: Context) -> anyhow::Result<SimpleOutput> {
    ctx.log("Starting simple task").await?;
    Ok(SimpleOutput {
        transformed_message: input.message.to_lowercase(),
    })
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let hatchet: Hatchet = Hatchet::from_env().await.unwrap();

    let simple_task = hatchet
        .task("simple-task", simple_task_func)
        .build()
        .unwrap();

    let input = SimpleInput {
        message: String::from("Hello, world!"),
    };

    // Run the task asynchronously, immediately returning the run ID
    let run_id = simple_task.run_no_wait(&input, None).await.unwrap();
    // Run the task synchronously, waiting for a worker to complete it and return the result
    let result = simple_task.run(&input, None).await.unwrap();
    println!("Result: {}", result.transformed_message);
}

```
## Workers
Workers are responsible for executing individual tasks.
### Declaring a Worker
Declare a worker by calling the worker method on the Hatchet client. Tasks and workflows can be added to the worker. When the worker starts
it will register the tasks with the Hatchet engine, allowing them to be triggered and assigned.
```rust no_run
use hatchet_sdk::{Context, EmptyModel, Hatchet, Register};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let hatchet = Hatchet::from_env().await.unwrap();

    async fn simple_task_func(input: EmptyModel, ctx: Context) -> anyhow::Result<serde_json::Value> {
        ctx.log("Starting simple task").await?;
        Ok(serde_json::json!({"message": "success"}))
    }

    let hatchet: Hatchet = Hatchet::from_env().await.unwrap();

    let simple_task = hatchet
        .task("simple-task", simple_task_func)
        .build()
        .unwrap();

    hatchet
        .worker("example-worker")
        .build()
        .unwrap()
        .add_task_or_workflow(&simple_task)
        .start()
        .await
        .unwrap();
}

```
## Declarative Workflow Design (DAGs)
Hatchet workflows are designed in a Directed Acyclic Graph (DAG) format,
where each task is a node in the graph, and the dependencies between tasks are the edges.
### Building a DAG with Task Dependencies
The power of Hatchet’s workflow design comes from connecting tasks into a DAG structure.
Tasks can specify dependencies (parents) which must complete successfully before the task can start.
### Running a Workflow
You can run workflows directly or enqueue them for asynchronous execution.
```rust no_run
use anyhow;
use hatchet_sdk::{Context, EmptyModel, Hatchet, Runnable};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct FirstTaskOutput {
    output: String,
}

#[derive(Serialize, Deserialize)]
struct SecondTaskOutput {
    first_step_result: String,
    final_result: String,
}

#[derive(Serialize, Deserialize)]
pub struct WorkflowOutput {
    first_task: FirstTaskOutput,
    second_task: SecondTaskOutput,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let hatchet = Hatchet::from_env().await.unwrap();

    let first_task = hatchet
        .task(
            "first_task",
            async move |_input: EmptyModel, _ctx: Context| -> anyhow::Result<FirstTaskOutput> {
                Ok(FirstTaskOutput {
                    output: "Hello World".to_string(),
                })
            },
        )
        .build()
        .unwrap();

    let second_task = hatchet
        .task(
            "second_task",
            async move |_input: EmptyModel, ctx: Context| -> anyhow::Result<SecondTaskOutput> {
                let first_result = ctx.parent_output("first_task").await?;
                Ok(SecondTaskOutput {
                    first_step_result: first_result.get("output").unwrap().to_string(),
                    final_result: "Completed".to_string(),
                })
            },
        )
        .build()
        .unwrap()
        .add_parent(&first_task);

    let workflow = hatchet
        .workflow::<EmptyModel, WorkflowOutput>("dag-workflow")
        .build()
        .unwrap()
        .add_task(&first_task)
        .add_task(&second_task);

    // Run the workflow asynchronously, immediately returning the run ID
    let run_id = workflow.run_no_wait(&EmptyModel, None).await.unwrap();
    // Run the workflow synchronously, waiting for a worker to complete it and return the result
    let result = workflow.run(&EmptyModel, None).await.unwrap();
    println!(
        "First task result: {}",
        serde_json::to_string(&result.first_task).unwrap()
    );
    println!(
        "Second task result: {}",
        serde_json::to_string(&result.second_task).unwrap()
    );
}

```

