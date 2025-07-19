use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tonic::Request;
use workflows::{TriggerWorkflowRequest, workflow_service_client::WorkflowServiceClient};

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::models::WorkflowStatus;

pub mod workflows {
    tonic::include_proto!("_");
}

pub struct Workflow<'a, I, O> {
    name: String,
    client: &'a HatchetClient,
    _input: PhantomData<I>,
    _output: PhantomData<O>,
}

impl<'a, I, O> Workflow<'a, I, O>
where
    I: Serialize,
    O: DeserializeOwned,
{
    pub fn new(name: impl Into<String>, client: &'a HatchetClient) -> Self {
        Self {
            name: name.into(),
            client,
            _input: PhantomData,
            _output: PhantomData,
        }
    }

    pub async fn run_no_wait(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<RunId, HatchetError> {
        self.trigger(input, options.unwrap_or_default()).await
    }

    pub async fn run(
        mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<O, HatchetError> {
        let run_id = self.run_no_wait(input, options).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        loop {
            let workflow = self.get_run(&run_id).await?;

            match workflow.run.status {
                WorkflowStatus::Running => continue,
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

                _ => {
                    // still running
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    pub async fn trigger<T>(
        &self,
        input: T,
        options: TriggerWorkflowOptions,
    ) -> Result<RunId, HatchetError>
    where
        T: Serialize,
    {
        let input_json = serde_json::to_string(&input).map_err(HatchetError::JsonEncode)?;

        let request = Request::new(TriggerWorkflowRequest {
            input: input_json,
            name: self.name.clone(),
            parent_id: None,
            parent_step_run_id: None,
            child_index: None,
            child_key: None,
            additional_metadata: options.additional_metadata.map(|v| v.to_string()),
            desired_worker_id: options.desired_worker_id,
            priority: None,
        });

        let response = self
            .client
            .grpc_unary_with_auth(request, |channel, request| async move {
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
