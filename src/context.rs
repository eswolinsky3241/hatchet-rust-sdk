use tokio::sync::mpsc;

use crate::HatchetError;
use crate::clients::client::HatchetClient;
use crate::features::runs::models::GetWorkflowRunResponse;

#[derive(Debug)]
pub struct Context {
    logger_tx: mpsc::Sender<String>,
    client: HatchetClient,
    workflow_run_id: String,
    step_run_id: String,
}

impl Context {
    pub(crate) async fn new(
        client: HatchetClient,
        workflow_run_id: &str,
        step_run_id: &str,
    ) -> Self {
        let mut client_clone = client.clone();
        let (tx, mut rx) = mpsc::channel::<String>(100);
        let step_run_id = step_run_id.to_string();
        let workflow_run_id = workflow_run_id.to_string();
        let step_run_id_clone = step_run_id.clone();

        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                client_clone
                    .event_client
                    .put_log(&step_run_id_clone, message)
                    .await
                    .unwrap();
            }
        });
        Self {
            logger_tx: tx,
            client: client,
            workflow_run_id,
            step_run_id,
        }
    }

    pub async fn parent_output(
        &self,
        parent_step_name: &str,
    ) -> Result<serde_json::Value, HatchetError> {
        let workflow_run = self.get_current_workflow().await?;

        let current_task = workflow_run
            .tasks
            .iter()
            .find(|task| task.task_external_id == self.step_run_id)
            .unwrap();

        let parent = current_task
            .input
            .parents
            .get(parent_step_name)
            .ok_or_else(|| HatchetError::ParentTaskNotFound {
                parent_step_name: parent_step_name.to_string(),
            })?;

        Ok(parent.0.clone())
    }

    pub async fn filter_payload(&self) -> Result<serde_json::Value, HatchetError> {
        let workflow_run = self.get_current_workflow().await?;

        let current_task = workflow_run
            .tasks
            .iter()
            .find(|task| task.task_external_id == self.step_run_id)
            .unwrap();

        Ok(current_task.input.triggers.filter_payload.clone())
    }

    /// Log a line to the Hatchet API. This will send the log line to the Hatchet API and return immediately.
    /// ```no_run
    /// use hatchet_sdk::{HatchetClient, EmptyModel};
    /// let hatchet = HatchetClient::from_env().await.unwrap();
    /// let task = hatchet.task("my-task", |_input: EmptyModel, ctx: Context| async move {
    ///     ctx.log("Hello, world!").await.unwrap();
    ///     Ok(EmptyModel)
    /// });
    /// ```
    pub async fn log(&self, message: &str) -> Result<(), HatchetError> {
        self.logger_tx.send(message.to_string()).await.unwrap();

        Ok(())
    }

    async fn get_current_workflow(&self) -> Result<GetWorkflowRunResponse, HatchetError> {
        self.client
            .workflow_rest_client
            .get(&self.workflow_run_id)
            .await
    }
}
