use super::ExtractRunnableOutput;
use crate::Hatchet;
use crate::clients::grpc::v1::workflows::{CreateTaskOpts, CreateWorkflowVersionRequest};
use crate::context::Context;
use crate::error::HatchetError;
use crate::features::runs::models::GetWorkflowRunResponse;
use crate::features::runs::models::WorkflowStatus;
use crate::runnables::TriggerWorkflowOptions;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type TaskResult = Pin<Box<dyn Future<Output = Result<serde_json::Value, TaskError>> + Send>>;

#[derive(Debug)]
pub enum TaskError {
    InputDeserialization(serde_json::Error),
    OutputSerialization(serde_json::Error),
    Execution(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::InputDeserialization(e) => write!(f, "Failed to deserialize input: {}", e),
            TaskError::OutputSerialization(e) => write!(f, "Failed to serialize output: {}", e),
            TaskError::Execution(e) => write!(f, "Task execution failed: {}", e),
        }
    }
}

impl std::error::Error for TaskError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TaskError::InputDeserialization(e) => Some(e),
            TaskError::OutputSerialization(e) => Some(e),
            TaskError::Execution(e) => Some(e.as_ref()),
        }
    }
}

pub trait ExecutableTask: Send + Sync + dyn_clone::DynClone {
    fn execute(&self, input: serde_json::Value, ctx: Context) -> TaskResult;
    fn name(&self) -> &str;
}

dyn_clone::clone_trait_object!(ExecutableTask);

#[derive(Clone)]
pub struct Task<I, O, E> {
    client: Hatchet,
    pub(crate) name: String,
    handler:
        Arc<dyn Fn(I, Context) -> Pin<Box<dyn Future<Output = Result<O, E>> + Send>> + Send + Sync>,
    parents: Vec<String>,
}

impl<I, O, E> Task<I, O, E>
where
    I: Serialize + for<'de> Deserialize<'de> + Send + 'static,
    O: Serialize + Send + 'static,
    E: Into<Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
{
    pub fn new<F, Fut>(name: impl Into<String>, handler: F, client: Hatchet) -> Self
    where
        F: FnOnce(I, Context) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<O, E>> + Send + 'static,
    {
        let name = name.into();
        let handler = Arc::new(move |input: I, ctx: Context| {
            let handler_clone = handler.clone();
            Box::pin(handler_clone(input, ctx))
                as Pin<Box<dyn Future<Output = Result<O, E>> + Send>>
        });

        Self {
            client,
            name,
            handler,
            parents: vec![],
        }
    }

    pub fn add_parent<J, P, F>(mut self, parent: &Task<J, P, F>) -> Self {
        self.parents.push(parent.name.clone());
        self
    }

    pub(crate) fn into_executable(self) -> Box<dyn ExecutableTask> {
        let handler = self.handler;
        let name = self.name;

        Box::new(TypeErasedTask {
            name: name.clone(),
            handler: Arc::new(
                move |input: serde_json::Value, ctx: Context| -> TaskResult {
                    let handler = handler.clone();
                    Box::pin(async move {
                        let typed_input: I = serde_json::from_value(input)
                            .map_err(TaskError::InputDeserialization)?;

                        let result = handler(typed_input, ctx)
                            .await
                            .map_err(|e| TaskError::Execution(e.into()))?;

                        serde_json::to_value(result).map_err(TaskError::OutputSerialization)
                    }) as TaskResult
                },
            ),
        })
    }

    pub(crate) fn to_task_proto(&self, workflow_name: &str) -> CreateTaskOpts {
        CreateTaskOpts {
            readable_id: self.name.clone(),
            action: format!("{workflow_name}:{}", &self.name),
            timeout: String::from(""),
            inputs: String::from("{{}}"),
            parents: self.parents.clone(),
            retries: 0,
            rate_limits: vec![],
            worker_labels: std::collections::HashMap::new(),
            backoff_factor: None,
            backoff_max_seconds: None,
            concurrency: vec![],
            conditions: None,
            schedule_timeout: None,
        }
    }

    pub(crate) fn to_standalone_workflow_proto(&self) -> CreateWorkflowVersionRequest {
        let task_proto = self.to_task_proto(&self.name);
        CreateWorkflowVersionRequest {
            name: self.name.clone(),
            description: String::from(""),
            version: String::from(""),
            event_triggers: vec![],
            cron_triggers: vec![],
            tasks: vec![task_proto],
            concurrency: None,
            cron_input: None,
            on_failure_task: None,
            sticky: None,
            default_priority: None,
            concurrency_arr: vec![],
            default_filters: vec![],
        }
    }
impl<I, O, E> ExtractRunnableOutput<O> for Task<I, O, E>
where
    I: Serialize + DeserializeOwned + Send + Sync + 'static,
    O: Serialize + DeserializeOwned + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    fn extract_output(&self, workflow: GetWorkflowRunResponse) -> Result<O, HatchetError> {
        let task_output = workflow
            .tasks
            .iter()
            .find(|task| task.action_id == Some(format!("{}:{}", &self.name, &self.name)))
            .and_then(|task| task.output.clone())
            .ok_or_else(|| HatchetError::MissingOutput)?;

        Ok(serde_json::from_value(task_output)
            .map_err(|e| HatchetError::JsonDecodeError(e.to_string()))?)
    }
}

#[async_trait::async_trait]
impl<I, O, E> super::Runnable<I, O> for Task<I, O, E>
where
    I: Serialize + DeserializeOwned + Send + Sync + 'static,
    O: Serialize + DeserializeOwned + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    async fn get_run(&self, run_id: &str) -> Result<GetWorkflowRunResponse, HatchetError> {
        self.client.workflow_rest_client.get(&run_id).await
    }
    async fn run_no_wait(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<String, HatchetError> {
        Ok(self.trigger(input, options.unwrap_or_default()).await?)
    }
}

#[derive(Clone)]
struct TypeErasedTask {
    name: String,
    handler: Arc<dyn Fn(serde_json::Value, Context) -> TaskResult + Send + Sync>,
}

impl ExecutableTask for TypeErasedTask {
    fn execute(&self, input: serde_json::Value, ctx: Context) -> TaskResult {
        (self.handler)(input, ctx)
    }

    fn name(&self) -> &str {
        &self.name
    }
}
