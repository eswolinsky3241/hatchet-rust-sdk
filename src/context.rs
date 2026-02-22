use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::mpsc;

use crate::{GetWorkflowRunResponse, Hatchet, HatchetError};

/// The context object is used to interact with the Hatchet API from within a task.
#[derive(Debug)]
pub struct Context {
    logger_tx: mpsc::Sender<String>,
    stream_tx: mpsc::Sender<(Vec<u8>, i64)>,
    stream_index: Arc<AtomicI64>,
    client: Hatchet,
    workflow_run_id: String,
    task_run_external_id: String,
}

impl Context {
    pub(crate) async fn new(
        client: Hatchet,
        workflow_run_id: &str,
        task_run_external_id: &str,
    ) -> Self {
        let task_run_external_id = task_run_external_id.to_string();
        let workflow_run_id = workflow_run_id.to_string();

        // Logger background drainer
        let mut log_client = client.clone();
        let (log_tx, mut log_rx) = mpsc::channel::<String>(100);
        let log_task_id = task_run_external_id.clone();
        tokio::spawn(async move {
            while let Some(message) = log_rx.recv().await {
                log_client
                    .event_client
                    .put_log(&log_task_id, message)
                    .await?;
            }
            Ok::<(), HatchetError>(())
        });

        // Stream background drainer
        let mut stream_client = client.clone();
        let (stream_tx, mut stream_rx) = mpsc::channel::<(Vec<u8>, i64)>(100);
        let stream_task_id = task_run_external_id.clone();
        tokio::spawn(async move {
            while let Some((message, index)) = stream_rx.recv().await {
                stream_client
                    .event_client
                    .put_stream_event(&stream_task_id, message, Some(index))
                    .await?;
            }
            Ok::<(), HatchetError>(())
        });

        Self {
            logger_tx: log_tx,
            stream_tx,
            stream_index: Arc::new(AtomicI64::new(0)),
            client,
            workflow_run_id,
            task_run_external_id,
        }
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
        self.logger_tx.send(message.to_string()).await.unwrap();

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
        self.stream_tx
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
