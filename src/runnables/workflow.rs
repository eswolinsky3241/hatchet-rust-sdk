use serde::Serialize;
use serde::de::DeserializeOwned;

use super::ExtractRunnableOutput;
use crate::clients::grpc::v1::workflows::{
    CreateTaskOpts, CreateWorkflowVersionRequest, DefaultFilter as DefaultFilterProto,
};
use crate::clients::hatchet::Hatchet;
use crate::error::HatchetError;
use crate::features::runs::models::GetWorkflowRunResponse;
use crate::runnables::task::{ExecutableTask, Task};
use derive_builder::Builder;

#[derive(Clone, Builder)]
#[builder(pattern = "owned")]
pub struct Workflow<I, O> {
    pub(crate) name: String,
    client: Hatchet,
    #[builder(default = vec![])]
    pub(crate) executable_tasks: Vec<Box<dyn ExecutableTask>>,
    #[builder(default = String::from(""))]
    description: String,
    #[builder(default = String::from(""))]
    version: String,
    #[builder(default = 1)]
    default_priority: i32,
    #[builder(default = vec![])]
    tasks: Vec<CreateTaskOpts>,
    #[builder(default = vec![])]
    on_events: Vec<String>,
    #[builder(default = vec![])]
    cron_triggers: Vec<String>,
    #[builder(default = vec![])]
    default_filters: Vec<DefaultFilter>,
    #[builder(default = std::marker::PhantomData)]
    _phantom: std::marker::PhantomData<(I, O)>,
}

impl<I, O> Workflow<I, O>
where
    I: Serialize + Send + Sync,
    O: DeserializeOwned + Send + Sync,
{
    pub fn add_task<P>(mut self, task: &Task<I, P>) -> Self
    where
        I: serde::de::DeserializeOwned + Send + 'static,
        P: serde::Serialize + Send + 'static,
    {
        if self
            .tasks
            .iter()
            .any(|existing_task| existing_task.readable_id == task.name)
        {
            panic!("Duplicate tasks registered to workflow: {}", task.name);
        }

        self.tasks.push(task.to_task_proto(&self.name));
        self.executable_tasks.push(task.into_executable());
        self
    }

    pub(crate) fn to_proto(&self) -> CreateWorkflowVersionRequest {
        CreateWorkflowVersionRequest {
            name: self.name.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            event_triggers: self.on_events.clone(),
            cron_triggers: self.cron_triggers.clone(),
            tasks: self.tasks.clone(),
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
        input: I,
        options: TriggerWorkflowOptions,
    ) -> Result<String, HatchetError> {
        let input_json =
            serde_json::to_value(&input).map_err(|e| HatchetError::JsonEncode(e.to_string()))?;

        let response = self
            .client
            .workflow_client
            .trigger_workflow(
                crate::clients::grpc::v0::workflows::TriggerWorkflowRequest {
                    name: self.name.clone(),
                    input: input_json.to_string(),
                    parent_id: None,
                    parent_step_run_id: None,
                    child_index: None,
                    child_key: None,
                    additional_metadata: options.additional_metadata.map(|v| v.to_string()),
                    desired_worker_id: None,
                    priority: None,
                },
            )
            .await?;

        Ok(response.workflow_run_id)
    }

    fn safely_get_action_name(&self, action_id: &str) -> Option<String> {
        action_id.split(':').nth(1).map(|s| s.to_string())
    }
}

impl<I, O> ExtractRunnableOutput<O> for Workflow<I, O>
where
    I: Serialize + Send + Sync + 'static,
    O: DeserializeOwned + Send + Sync + 'static,
{
    fn extract_output(&self, workflow: GetWorkflowRunResponse) -> Result<O, HatchetError> {
        let mut task_outputs = serde_json::Map::new();

        for task in &workflow.tasks {
            if let (Some(action_id), Some(output)) = (&task.action_id, &task.output) {
                if let Some(task_name) = self.safely_get_action_name(action_id) {
                    task_outputs.insert(task_name, output.clone());
                }
            }
        }

        let output_value = serde_json::Value::Object(task_outputs);
        Ok(serde_json::from_value(output_value)
            .map_err(|e| HatchetError::JsonDecodeError(e.to_string()))?)
    }
}

#[async_trait::async_trait]
impl<I, O> crate::runnables::Runnable<I, O> for Workflow<I, O>
where
    I: Serialize + Send + Sync + DeserializeOwned + 'static,
    O: DeserializeOwned + Send + Sync + 'static,
{
    async fn get_run(&self, run_id: &str) -> Result<GetWorkflowRunResponse, HatchetError> {
        self.client.workflow_rest_client.get(&run_id).await
    }

    async fn run_no_wait(
        &self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<String, HatchetError> {
        Ok(self.trigger(input, options.unwrap_or_default()).await?)
    }
}

#[derive(Debug, Default, Clone)]
pub struct TriggerWorkflowOptions {
    pub additional_metadata: Option<serde_json::Value>,
    pub desired_worker_id: Option<String>,
    pub namespace: Option<String>,
    pub sticky: bool,
    pub key: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct DefaultFilter {
    pub expression: String,
    pub scope: String,
    pub payload: Option<serde_json::Value>,
}

impl DefaultFilter {
    pub fn new(expression: String, scope: String, payload: Option<serde_json::Value>) -> Self {
        Self {
            expression,
            scope,
            payload,
        }
    }
}

impl DefaultFilter {
    pub fn to_proto(&self) -> DefaultFilterProto {
        DefaultFilterProto {
            expression: self.expression.clone(),
            scope: self.scope.clone(),
            payload: self.payload.clone().map(|v| v.to_string().into()),
        }
    }
}
