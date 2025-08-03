use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tonic::Request;

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::grpc::workflows::workflow_service_client::WorkflowServiceClient;
use crate::grpc::workflows::{
    CreateWorkflowJobOpts, CreateWorkflowStepOpts, CreateWorkflowVersionOpts,
    TriggerWorkflowRequest,
};
use crate::models::WorkflowStatus;
use crate::utils::{EXECUTION_CONTEXT, ExecutionContext};
use crate::workflows::task::Task;

pub struct Workflow<I, O> {
    name: String,
    client: Arc<HatchetClient>,
    steps: Vec<CreateWorkflowStepOpts>,
    actions: Vec<String>,
    _input: PhantomData<I>,
    _output: PhantomData<O>,
}

impl<I, O> Workflow<I, O>
where
    I: Serialize,
    O: DeserializeOwned,
{
    pub fn new(name: impl Into<String>, client: Arc<HatchetClient>) -> Self {
        Self {
            name: name.into(),
            client,
            steps: vec![],
            _input: PhantomData,
            _output: PhantomData,
        }
    }

    pub fn add_task<T>(&mut self, task: &Task<T>) -> () {
        self.steps.push(task.to_proto(&self.name));
    }

    pub(crate) fn to_proto(&self) -> CreateWorkflowVersionOpts {
        CreateWorkflowVersionOpts {
            name: self.name.clone(),
            description: String::from(""),
            version: String::from(""),
            event_triggers: vec![],
            cron_triggers: vec![],
            scheduled_triggers: vec![],
            jobs: vec![CreateWorkflowJobOpts {
                name: String::from("job"),
                description: String::from(""),
                steps: self.steps.clone(),
            }],
            concurrency: None,
            schedule_timeout: None,
            cron_input: None,
            on_failure_job: None,
            sticky: None,
            kind: None,
            default_priority: None,
        }
    }

    pub async fn run_no_wait(
        &self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<RunId, HatchetError> {
        self.trigger(input, options.unwrap_or_default()).await
    }

    pub async fn run(
        self,
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
                    let output_json = workflow
                        .tasks
                        .last() // Get the output of the last task
                        .ok_or(HatchetError::MissingTasks)?
                        .output
                        .as_ref()
                        .ok_or(HatchetError::MissingOutput)?;
                    let output: O = serde_json::from_value(output_json.clone())
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
        &self,
        input: T,
        options: TriggerWorkflowOptions,
    ) -> Result<RunId, HatchetError>
    where
        T: Serialize,
    {
        let input_json = serde_json::to_string(&input).map_err(HatchetError::JsonEncode)?;

        let mut request = TriggerWorkflowRequest {
            input: input_json,
            name: self.name.clone(),
            parent_id: None,
            parent_step_run_id: None,
            child_index: None,
            child_key: None,
            additional_metadata: options.additional_metadata.map(|v| v.to_string()),
            desired_worker_id: options.desired_worker_id,
            priority: None,
        };

        Self::update_task_execution_context(&mut request);

        let response = self
            .client
            .grpc_unary(Request::new(request), |channel, request| async move {
                let mut client = WorkflowServiceClient::new(channel);
                client.trigger_workflow(request).await
            })
            .await?;

        Ok(RunId(response.into_inner().workflow_run_id))
    }

    pub async fn get_run(
        &self,
        run_id: &RunId,
    ) -> Result<crate::models::GetWorkflowRunResponse, HatchetError> {
        self.client
            .api_get(&format!("/api/v1/stable/workflow-runs/{}", run_id))
            .await
    }

    fn update_task_execution_context(request: &mut TriggerWorkflowRequest) {
        if let Ok(ctx) = EXECUTION_CONTEXT.try_with(|c| c.clone()) {
            let ctx_inner: ExecutionContext = ctx.into_inner();
            request.child_index = Some(ctx_inner.child_index.clone());
            request.parent_id = Some(ctx_inner.workflow_run_id.clone());
            request.parent_step_run_id = Some(ctx_inner.step_run_id.clone());
            EXECUTION_CONTEXT.with(|ctx| {
                let mut ctx = ctx.borrow_mut();
                ctx.child_index += 1;
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunId(pub String);

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for RunId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
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
