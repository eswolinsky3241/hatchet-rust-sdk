 # Hatchet SDK for Rust.

 This is an unofficial Rust SDK for [Hatchet](https://hatchet.run), a distributed, fault-tolerant task queue.
 This crate allows you to integrate Hatchet into your Rust applications.

 ## Examples

 We recommend adding your Hatchet API token to a `.env` file and installing [dotenvy](https://crates.io/crates/dotenvy) to load it in your application.

 ### Defining a simple workflow

 ```rust
 use hatchet_sdk::{Context, Hatchet};
 use serde::{Deserialize, Serialize};

 // Define your input and output types
 #[derive(Serialize, Deserialize)]
 struct SimpleInput {
     message: String,
 }

 #[derive(Serialize, Deserialize, Debug)]
 struct SimpleOutput {
     transformed_message: String,
 }

 // Define your task handler
 async fn simple_task(input: SimpleInput, ctx: Context) -> anyhow::Result<SimpleOutput> {
     ctx.log("Starting simple task").await?;
     Ok(SimpleOutput {
         transformed_message: input.message.to_lowercase(),
     })
 }

 #[tokio::main]
 async fn main() {
     // Load the .env file
     dotenvy::dotenv().ok();

     // Create a Hatchet client
     let hatchet = Hatchet::from_env().await.unwrap();

     // Create a workflow
     let mut workflow = hatchet.workflow::<SimpleInput, SimpleOutput>()
         .name(String::from("simple-workflow"))
         .build()
         .unwrap()
         .add_task(hatchet.task("simple-task", simple_task))
         .unwrap();

     // Create and start a worker, registering the workflow with Hatchet
     hatchet.worker()
         .name(String::from("simple-worker"))
         .max_runs(5)
         .build()
         .unwrap()
         .add_workflow(workflow)
         .start()
         .await
         .unwrap();
 }
 ```

 ### Running workflows

 Use the `run` method to run the workflow synchronously:

 ```rust
 let output = workflow.run(SimpleInput {
     message: "Hello, world".to_string(),
 }, None).await.unwrap();

 println("Output: {:?}", output);
 ```

 Use the `run_no_wait` method to run the workflow asynchronously:

 ```rust
 workflow.run_no_wait(SimpleInput {
     message: "Hello, world".to_string(),
 }, None).await.unwrap();
 ```

