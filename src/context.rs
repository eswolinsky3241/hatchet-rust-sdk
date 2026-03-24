use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;

use crate::{GetWorkflowRunResponse, Hatchet, HatchetError};

/// The context object is used to interact with the Hatchet API from within a task.
pub struct Context {
    log_tx: OnceLock<mpsc::Sender<String>>,
    stream_tx: OnceLock<mpsc::Sender<(Vec<u8>, i64)>>,
    stream_index: Arc<AtomicI64>,
    client: Hatchet,
    workflow_run_id: String,
    task_run_external_id: String,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("workflow_run_id", &self.workflow_run_id)
            .field("task_run_external_id", &self.task_run_external_id)
            .finish()
    }
}

impl Context {
    pub(crate) fn new(
        client: Hatchet,
        workflow_run_id: &str,
        task_run_external_id: &str,
    ) -> Self {
        Self {
            log_tx: OnceLock::new(),
            stream_tx: OnceLock::new(),
            stream_index: Arc::new(AtomicI64::new(0)),
            client,
            workflow_run_id: workflow_run_id.to_string(),
            task_run_external_id: task_run_external_id.to_string(),
        }
    }

    fn get_or_init_log_tx(&self) -> &mpsc::Sender<String> {
        self.log_tx.get_or_init(|| {
            let mut log_client = self.client.clone();
            let (tx, mut rx) = mpsc::channel::<String>(100);
            let task_id = self.task_run_external_id.clone();
            tokio::spawn(async move {
                while let Some(message) = rx.recv().await {
                    let _ = log_client.event_client.put_log(&task_id, message).await;
                }
            });
            tx
        })
    }

    fn get_or_init_stream_tx(&self) -> &mpsc::Sender<(Vec<u8>, i64)> {
        self.stream_tx.get_or_init(|| {
            let mut stream_client = self.client.clone();
            let (tx, mut rx) = mpsc::channel::<(Vec<u8>, i64)>(100);
            let task_id = self.task_run_external_id.clone();
            tokio::spawn(async move {
                while let Some((message, index)) = rx.recv().await {
                    let _ = stream_client
                        .event_client
                        .put_stream_event(&task_id, message, Some(index))
                        .await;
                }
            });
            tx
        })
    }

    /// Get the output of a parent task in a DAG.
    ///
    /// ```compile_fail
    /// let task = hatchet.task("my-task", |_input: EmptyModel, ctx: Context| async move {
    ///     let parent_output = ctx.parent_output("parent_task").await.unwrap();
    ///     Ok(EmptyModel)
    /// });
    /// ```
    pub async fn parent_output(
        &self,
        parent_step_name: &str,
    ) -> Result<serde_json::Value, HatchetError> {
        let workflow_run = self.get_current_workflow().await?;

        let current_task = workflow_run
            .tasks
            .iter()
            .find(|task| task.task_external_id == self.task_run_external_id)
            .ok_or_else(|| HatchetError::ParentTaskNotFound {
                parent_step_name: parent_step_name.to_string(),
            })?;

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
            .find(|task| task.task_external_id == self.task_run_external_id)
            .unwrap();

        Ok(current_task.input.triggers.filter_payload.clone())
    }

    /// Log a line to the Hatchet API. This will send the log line to the Hatchet API and return immediately.
    /// ```compile_fail
    /// use hatchet_sdk::{Hatchet, EmptyModel};
    /// let hatchet = Hatchet::from_env().await.unwrap();
    /// let task = hatchet.task("my-task", |_input: EmptyModel, ctx: Context| async move {
    ///     ctx.log("Hello, world!").await.unwrap();
    ///     Ok(EmptyModel)
    /// });
    /// ```
    pub async fn log(&self, message: &str) -> Result<(), HatchetError> {
        self.get_or_init_log_tx()
            .send(message.to_string())
            .await
            .map_err(|e| HatchetError::InternalError(e.to_string()))?;
        Ok(())
    }

    /// Send a stream event to the Hatchet API. Useful for streaming data such as LLM tokens
    /// or progress updates from a task to consumers.
    ///
    /// ```compile_fail
    /// use hatchet_sdk::{Hatchet, EmptyModel};
    /// let hatchet = Hatchet::from_env().await.unwrap();
    /// let task = hatchet.task("my-task", |_input: EmptyModel, ctx: Context| async move {
    ///     ctx.put_stream(b"chunk 1".to_vec()).await.unwrap();
    ///     ctx.put_stream(b"chunk 2".to_vec()).await.unwrap();
    ///     Ok(EmptyModel)
    /// });
    /// ```
    pub async fn put_stream(&self, data: impl Into<Vec<u8>>) -> Result<(), HatchetError> {
        let index = self.stream_index.fetch_add(1, Ordering::SeqCst);
        self.get_or_init_stream_tx()
            .send((data.into(), index))
            .await
            .map_err(|e| HatchetError::StreamError(e.to_string()))?;
        Ok(())
    }

    async fn get_current_workflow(&self) -> Result<GetWorkflowRunResponse, HatchetError> {
        self.client
            .workflow_rest_client
            .get(&self.workflow_run_id)
            .await
    }
}
