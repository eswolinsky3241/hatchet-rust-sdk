use tokio::sync::mpsc;

use crate::HatchetError;
use crate::client::HatchetClientTrait;
use crate::rest::models::GetWorkflowRunResponse;

pub trait HatchetContextTrait: Clone + Send + Sync + 'static {
    async fn parent_output(
        &self,
        parent_step_name: &str,
    ) -> Result<serde_json::Value, HatchetError>;

    async fn filter_payload(&self) -> Result<serde_json::Value, HatchetError>;

    async fn log(&self, message: String) -> Result<(), HatchetError>;
}

#[derive(Clone, Debug)]
pub struct Context<C> {
    logger_tx: mpsc::Sender<String>,
    client: C,
    workflow_run_id: String,
    step_run_id: String,
}

impl<C> Context<C>
where
    C: HatchetClientTrait,
{
    pub(crate) async fn new(client: C, workflow_run_id: &str, step_run_id: &str) -> Self {
        let mut client_clone = client.clone();
        let (tx, mut rx) = mpsc::channel::<String>(100);
        let step_run_id = step_run_id.to_string();
        let workflow_run_id = workflow_run_id.to_string();
        let step_run_id_clone = step_run_id.clone();

        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                client_clone
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

    async fn get_current_workflow(&self) -> Result<GetWorkflowRunResponse, HatchetError> {
        self.client
            .api_get(&format!(
                "/api/v1/stable/workflow-runs/{}",
                self.workflow_run_id
            ))
            .await
    }
}

impl<C> HatchetContextTrait for Context<C>
where
    C: HatchetClientTrait,
{
    async fn parent_output(
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

    async fn filter_payload(&self) -> Result<serde_json::Value, HatchetError> {
        let workflow_run = self.get_current_workflow().await?;

        let current_task = workflow_run
            .tasks
            .iter()
            .find(|task| task.task_external_id == self.step_run_id)
            .unwrap();

        Ok(current_task.input.triggers.filter_payload.clone())
    }

    async fn log(&self, message: String) -> Result<(), HatchetError> {
        self.logger_tx.send(message).await.unwrap();

        Ok(())
    }
}
