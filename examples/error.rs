use hatchet_sdk::{Context, EmptyModel, Hatchet, Runnable};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyCustomError {
    #[error("Task failed. Start worker with RUST_BACKTRACE=1 to see backtrace")]
    TaskFailed,
}

pub async fn create_error_task() -> hatchet_sdk::Task<EmptyModel, EmptyModel> {
    let hatchet = Hatchet::from_env().await.unwrap();
    hatchet
        .task(
            "error-task",
            async move |_input: EmptyModel, _ctx: Context| -> anyhow::Result<EmptyModel> {
                Err(MyCustomError::TaskFailed)?
            },
        )
        .build()
        .unwrap()
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();
    let error_task = create_error_task().await;
    match error_task.run(&EmptyModel, None).await {
        Ok(_) => (),
        Err(error) => {
            println!("Error: {}", error);
        }
    };
}
