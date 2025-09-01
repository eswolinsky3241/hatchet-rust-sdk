use super::workflow::TriggerWorkflowOptions;
use crate::error::HatchetError;
use serde::Serialize;
use serde::de::DeserializeOwned;

#[async_trait::async_trait]
pub trait Runnable<I, O>
where
    I: Serialize + Send + Sync,
    O: DeserializeOwned + Send + Sync,
{
    async fn run(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<O, HatchetError>;

    async fn run_no_wait(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<String, HatchetError>;
}
