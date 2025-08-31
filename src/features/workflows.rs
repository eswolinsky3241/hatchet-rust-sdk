use crate::clients::rest::apis::configuration::Configuration;
use crate::clients::rest::apis::workflow_runs_api::v1_workflow_run_get;
use crate::error::HatchetError;
use models::*;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct WorkflowsClient {
    configuration: Arc<Configuration>,
}

impl WorkflowsClient {
    pub fn new(configuration: Arc<Configuration>) -> Self {
        Self { configuration }
    }

    pub async fn get(&self, workflow_run_id: &str) -> Result<GetWorkflowRunResponse, HatchetError> {
        let response = v1_workflow_run_get(&self.configuration, workflow_run_id)
            .await
            .map_err(|e| HatchetError::RestApiError(e.to_string()))?;
        Ok(
            serde_json::from_str::<GetWorkflowRunResponse>(&serde_json::to_string(&response)?)
                .unwrap(),
        )
    }
}

pub mod models {
    use std::collections::HashMap;

    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Debug, Deserialize)]
    pub struct GetWorkflowRunResponse {
        pub tasks: Vec<Task>,
        pub run: Run,
    }

    #[derive(Debug, Deserialize)]
    pub struct TaskParent(pub Value);

    #[derive(Debug, Deserialize)]
    pub struct Triggers {
        pub filter_payload: serde_json::Value,
    }

    #[derive(Debug, Deserialize)]
    pub struct TaskInput {
        pub parents: HashMap<String, TaskParent>,
        pub triggers: Triggers,
    }

    #[derive(Debug, Deserialize)]
    pub struct Task {
        pub output: Option<Value>,
        pub input: TaskInput,
        #[serde(rename = "taskExternalId")]
        pub task_external_id: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct Run {
        pub status: WorkflowStatus,
        #[serde(rename = "errorMessage")]
        pub error_message: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "UPPERCASE")]
    pub enum WorkflowStatus {
        Running,
        Failed,
        Completed,
        Queued,
        Cancelled,
        #[serde(other)]
        Unknown,
    }

    #[derive(Debug, Deserialize)]
    pub struct Workflow;
}
