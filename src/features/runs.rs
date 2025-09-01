use crate::clients::rest::apis::configuration::Configuration;
use crate::clients::rest::apis::workflow_runs_api::v1_workflow_run_get;
use crate::error::HatchetError;
use models::*;
use std::sync::Arc;

///The runs client is a client for interacting with task and workflow runs within Hatchet.
#[derive(Clone, Debug)]
pub struct RunsClient {
    configuration: Arc<Configuration>,
}

impl RunsClient {
    pub fn new(configuration: Arc<Configuration>) -> Self {
        Self { configuration }
    }

    /// Get a workflow run by its ID.
    ///
    /// ```no_run
    /// use hatchet_sdk::{HatchetClient, EmptyModel};
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = HatchetClient::from_env().await.unwrap();
    ///     let workflow_run = hatchet.workflow_rest_client.get("123").await.unwrap();
    /// }
    /// ```
    pub async fn get(&self, workflow_run_id: &str) -> Result<GetWorkflowRunResponse, HatchetError> {
        let response = v1_workflow_run_get(&self.configuration, workflow_run_id)
            .await
            .map_err(|e| HatchetError::RestApiError(e.to_string()))?;
        Ok(GetWorkflowRunResponse::from(response))
    }
}

pub mod models {
    use crate::clients::rest::models::V1WorkflowRunCreate200Response;
    use serde::Deserialize;
    use serde_json::Value;
    use std::collections::HashMap;

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

    impl From<V1WorkflowRunCreate200Response> for GetWorkflowRunResponse {
        fn from(response: V1WorkflowRunCreate200Response) -> Self {
            let json_str = serde_json::to_string(&response)
                .expect("Failed to serialize V1WorkflowRunCreate200Response");
            serde_json::from_str(&json_str)
                .expect("Failed to deserialize to GetWorkflowRunResponse")
        }
    }
}
