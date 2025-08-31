use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::clients::client::HatchetClient;
use crate::clients::grpc::v1::workflows::{
    CreateTaskOpts, CreateWorkflowVersionRequest, DefaultFilter as DefaultFilterProto,
};
use crate::error::HatchetError;
use crate::features::workflows::models::GetWorkflowRunResponse;
use crate::features::workflows::models::WorkflowStatus;
use crate::workflows::task::{ExecutableTask, Task};
use derive_builder::Builder;

#[derive(Clone, Builder)]
#[builder(pattern = "owned")]
pub struct Workflow<I, O> {
    pub(crate) name: String,
    client: HatchetClient,
    #[builder(default = "vec![]")]
    pub(crate) executable_tasks: Vec<Box<dyn ExecutableTask>>,
    #[builder(default = "vec![]")]
    tasks: Vec<CreateTaskOpts>,
    #[builder(default = "vec![]")]
    on_events: Vec<String>,
    #[builder(default = "vec![]")]
    cron_triggers: Vec<String>,
    #[builder(default = "vec![]")]
    default_filters: Vec<DefaultFilter>,
    #[builder(default = "std::marker::PhantomData")]
    _phantom: std::marker::PhantomData<(I, O)>,
}

impl<I, O> Workflow<I, O>
where
    I: Serialize + Send + Sync + Clone,
    O: DeserializeOwned + Send + Sync + std::fmt::Debug + Clone,
{
    pub fn new(
        name: impl Into<String>,
        client: HatchetClient,
        on_events: Vec<String>,
        cron_triggers: Vec<String>,
        default_filters: Vec<DefaultFilter>,
    ) -> Self {
        Self {
            name: name.into(),
            client,
            executable_tasks: vec![],
            tasks: vec![],
            on_events,
            cron_triggers: cron_triggers,
            default_filters,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn add_task<P, E>(mut self, task: Task<I, P, E>) -> Result<Self, HatchetError>
    where
        I: serde::de::DeserializeOwned + Send + 'static,
        P: serde::Serialize + Send + 'static,
        E: Into<Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
    {
        if self
            .tasks
            .iter()
            .any(|existing_task| existing_task.readable_id == task.name)
        {
            return Err(HatchetError::DuplicateTask {
                task_name: task.name.clone(),
                workflow_name: self.name.clone(),
            });
        }

        self.tasks.push(task.to_proto(&self.name));
        self.executable_tasks.push(task.into_executable());
        Ok(self)
    }

    pub(crate) fn to_proto(&self) -> CreateWorkflowVersionRequest {
        CreateWorkflowVersionRequest {
            name: self.name.clone(),
            description: String::from(""),
            version: String::from(""),
            event_triggers: self.on_events.clone(),
            cron_triggers: self.cron_triggers.clone(),
            tasks: self.tasks.clone(),
            concurrency: None,
            cron_input: None,
            on_failure_task: None,
            sticky: None,
            default_priority: None,
            concurrency_arr: vec![],
            default_filters: self
                .default_filters
                .clone()
                .into_iter()
                .map(|f| f.to_proto())
                .collect(),
        }
    }

    pub async fn run_no_wait(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<String, HatchetError> {
        self.trigger(input, options.unwrap_or_default()).await
    }

    pub async fn run(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<O, HatchetError> {
        let run_id = self.run_no_wait(input, options).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        loop {
            let workflow = self.get_run(&run_id).await?;

            match workflow.run.status {
                WorkflowStatus::Running => {}
                WorkflowStatus::Completed => {
                    let output_json = &workflow
                        .tasks
                        .last() // Get the output of the last task
                        .ok_or(HatchetError::MissingTasks)?
                        .output
                        .as_ref()
                        .ok_or(HatchetError::MissingOutput)?
                        .to_string();
                    let output: O = serde_json::from_str(&output_json)
                        .map_err(|e| HatchetError::JsonDecodeError(e))?;
                    return Ok(output);
                }
                WorkflowStatus::Failed => {
                    return Err(HatchetError::WorkflowFailed {
                        error_message: workflow.run.error_message.clone(),
                    });
                }
                _ => {}
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    async fn trigger<T>(
        &mut self,
        input: T,
        options: TriggerWorkflowOptions,
    ) -> Result<String, HatchetError>
    where
        T: Serialize,
    {
        let input_json = serde_json::to_value(&input).map_err(HatchetError::JsonEncode)?;

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

    async fn get_run(&self, run_id: &str) -> Result<GetWorkflowRunResponse, HatchetError> {
        self.client.workflow_rest_client.get(&run_id).await
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
