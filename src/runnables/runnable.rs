use super::workflow::TriggerWorkflowOptions;
use crate::error::HatchetError;
use crate::features::runs::models::GetWorkflowRunResponse;
use crate::features::runs::models::WorkflowStatus;
use serde::Serialize;

use serde::de::DeserializeOwned;

#[async_trait::async_trait]
pub trait Runnable<I, O>: ExtractRunnableOutput<O> + Send + Sync
where
    I: Serialize + Send + Sync + DeserializeOwned + 'static,
    O: DeserializeOwned + Send + Sync + 'static,
{
    async fn get_run(&self, run_id: &str) -> Result<GetWorkflowRunResponse, HatchetError>;

    async fn run(
        &self,
        input: &I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<O, HatchetError> {
        let run_id = self.run_no_wait(input, options).await?;

        // Wait 2 seconds for eventual consistency
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        loop {
            let workflow = self.get_run(&run_id).await?;
            match workflow.run.status {
                WorkflowStatus::Running => {}
                WorkflowStatus::Completed => {
                    return Ok(self.extract_output(workflow)?);
                }
                WorkflowStatus::Failed => {
                    return Err(HatchetError::WorkflowFailed(
                        workflow.run.error_message.clone(),
                    ));
                }
                _ => {}
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    async fn run_no_wait(
        &self,
        input: &I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<String, HatchetError>;
}

pub trait ExtractRunnableOutput<O> {
    fn extract_output(&self, runnable: GetWorkflowRunResponse) -> Result<O, HatchetError>;
}
