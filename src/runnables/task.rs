use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::workflow::DefaultFilter;
use super::{ExtractRunnableOutput, TriggerWorkflowOptions};
use crate::clients::grpc::v1::workflows::{CreateTaskOpts, CreateWorkflowVersionRequest};
use crate::utils::duration_to_expr;
use crate::{Context, GetWorkflowRunResponse, Hatchet, HatchetError};

pub type TaskResult = Pin<Box<dyn Future<Output = Result<serde_json::Value, TaskError>> + Send>>;

#[derive(Debug)]
pub enum TaskError {
    InputDeserialization(serde_json::Error),
    OutputSerialization(serde_json::Error),
    Execution(anyhow::Error),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::InputDeserialization(e) => write!(f, "Failed to deserialize input: {}", e),
            TaskError::OutputSerialization(e) => write!(f, "Failed to serialize output: {}", e),
            TaskError::Execution(e) => {
                let error_message = format!("Task execution failed: {}", e);
                if std::env::var("RUST_BACKTRACE").is_ok_and(|v| v != "0") {
                    write!(f, "{}\n\n{}", error_message, e.backtrace())
                } else {
                    write!(f, "{}", error_message)
                }
            }
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

/// A task is a unit of work that can be executed by a worker.
/// See [Hatchet.task()](crate::Hatchet::task()) for more information.
#[derive(Clone, derive_builder::Builder)]
#[builder(pattern = "owned")]
pub struct Task<I, O> {
    client: Hatchet,
    pub(crate) name: String,
    handler: Arc<
        dyn Fn(I, Context) -> Pin<Box<dyn Future<Output = anyhow::Result<O>> + Send>> + Send + Sync,
    >,
    #[builder(default = vec![])]
    parents: Vec<String>,
    #[builder(default = String::from(""))]
    description: String,
    #[builder(default = String::from(""))]
    version: String,
    #[builder(default = 1)]
    default_priority: i32,
    #[builder(default = vec![])]
    on_events: Vec<String>,
    #[builder(default = vec![])]
    cron_triggers: Vec<String>,
    #[builder(default = vec![])]
    default_filters: Vec<DefaultFilter>,
    #[builder(default = 0)]
    retries: i32,
    #[builder(default = std::time::Duration::from_secs(300))]
    schedule_timeout: std::time::Duration,
    #[builder(default = std::time::Duration::from_secs(60))]
    execution_timeout: std::time::Duration,
}

impl<I, O> Task<I, O>
where
    I: Serialize + for<'de> Deserialize<'de> + Send + 'static,
    O: Serialize + Send + 'static,
{
    pub fn add_parent<J, P>(mut self, parent: &Task<J, P>) -> Self {
        self.parents.push(parent.name.clone());
        self
    }

    pub(crate) fn into_executable(&self) -> Box<dyn ExecutableTask> {
        let handler = self.handler.clone();
        let name = self.name.clone();

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
                            .map_err(TaskError::Execution)?;

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
            timeout: duration_to_expr(self.execution_timeout.clone()),
            inputs: String::from("{{}}"),
            parents: self.parents.clone(),
            retries: self.retries.clone(),
            rate_limits: vec![],
            worker_labels: std::collections::HashMap::new(),
            backoff_factor: None,
            backoff_max_seconds: None,
            concurrency: vec![],
            conditions: None,
            schedule_timeout: Some(duration_to_expr(self.schedule_timeout.clone())),
        }
    }

    pub(crate) fn to_standalone_workflow_proto(&self) -> CreateWorkflowVersionRequest {
        let task_proto = self.to_task_proto(&self.name);
        CreateWorkflowVersionRequest {
            name: self.name.clone().to_lowercase(),
            description: self.description.clone(),
            version: self.version.clone(),
            event_triggers: self.on_events.clone(),
            cron_triggers: self.cron_triggers.clone(),
            tasks: vec![task_proto],
            concurrency: None,
            cron_input: None,
            on_failure_task: None,
            sticky: None,
            default_priority: Some(self.default_priority),
            concurrency_arr: vec![],
            default_filters: self
                .default_filters
                .clone()
                .into_iter()
                .map(|f| f.to_proto())
                .collect(),
        }
    }

    async fn trigger(
        &self,
        input: &I,
        options: &TriggerWorkflowOptions,
    ) -> Result<String, HatchetError> {
        let input_json =
            serde_json::to_value(&input).map_err(|e| HatchetError::JsonEncode(e.to_string()))?;

        let additional_metadata = options.additional_metadata.clone().map(|v| v.to_string());
        let desired_worker_id = options.desired_worker_id.clone();

        let response = self
            .client
            .workflow_client
            .trigger_workflow(
                crate::clients::grpc::v0::workflows::TriggerWorkflowRequest {
                    name: self.name.clone().to_lowercase(),
                    input: input_json.to_string(),
                    parent_id: None,
                    parent_step_run_id: None,
                    child_index: None,
                    child_key: None,
                    additional_metadata,
                    desired_worker_id,
                    priority: None,
                },
            )
            .await?;

        Ok(response.workflow_run_id)
    }
}

impl<I, O> ExtractRunnableOutput<O> for Task<I, O>
where
    I: Serialize + DeserializeOwned + Send + Sync + 'static,
    O: Serialize + DeserializeOwned + Send + Sync + 'static,
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
impl<I, O> super::Runnable<I, O> for Task<I, O>
where
    I: Serialize + DeserializeOwned + Send + Sync + 'static,
    O: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    async fn get_run(&self, run_id: &str) -> Result<GetWorkflowRunResponse, HatchetError> {
        self.client.workflow_rest_client.get(&run_id).await
    }
    async fn run_no_wait(
        &self,
        input: &I,
        options: Option<&TriggerWorkflowOptions>,
    ) -> Result<String, HatchetError> {
        Ok(self
            .trigger(input, options.unwrap_or(&TriggerWorkflowOptions::default()))
            .await?)
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
